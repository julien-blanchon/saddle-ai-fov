use std::{collections::HashSet, time::Instant};

use bevy::{
    color::palettes::css,
    ecs::system::SystemState,
    gizmos::{gizmos::GizmoStorage, prelude::Gizmos},
    math::{Isometry2d, Isometry3d},
    prelude::*,
};

use crate::{
    FovRuntimeConfig,
    algorithms::shadowcasting::compute_grid_fov,
    components::{
        FovDirty, FovOccluder, FovTarget, GridFov, GridFovState, SpatialFov, SpatialFovState,
    },
    debug::{FovDebugGizmos, FovDebugSettings},
    grid::GridOpacityMap,
    resources::FovStats,
    spatial::{
        SpatialDimension, SpatialShape, SpatialVisibilityQuery, VisibilityLayerMask, WorldOccluder,
        evaluate_visibility, occluded_by_any,
    },
};

#[derive(Default, Resource, Debug)]
pub(crate) struct FovRuntimeState {
    pub active: bool,
}

pub(crate) fn activate_runtime(
    mut commands: Commands,
    mut runtime: ResMut<FovRuntimeState>,
    viewers: Query<Entity, Or<(With<GridFov>, With<SpatialFov>)>>,
) {
    runtime.active = true;
    for entity in &viewers {
        commands.entity(entity).insert(FovDirty);
    }
}

pub(crate) fn deactivate_runtime(
    mut commands: Commands,
    mut runtime: ResMut<FovRuntimeState>,
    viewers: Query<Entity, Or<(With<GridFov>, With<SpatialFov>)>>,
    mut grid_states: Query<&mut GridFovState>,
    mut spatial_states: Query<&mut SpatialFovState>,
) {
    runtime.active = false;

    for entity in &viewers {
        commands.entity(entity).remove::<FovDirty>();

        if let Ok(mut state) = grid_states.get_mut(entity) {
            if !state.visible_now.is_empty() {
                state.exited = state.visible_now.clone();
                state.visible_now.clear();
            }
        }

        if let Ok(mut state) = spatial_states.get_mut(entity) {
            if !state.visible_now.is_empty() {
                state.exited = state.visible_now.clone();
                state.visible_now.clear();
            }
        }
    }
}

pub(crate) fn runtime_is_active(runtime: Res<FovRuntimeState>) -> bool {
    runtime.active
}

pub(crate) fn debug_enabled(debug: Res<FovDebugSettings>) -> bool {
    debug.enabled
}

pub(crate) fn prepare_runtime(
    mut commands: Commands,
    viewers: Query<
        (
            Entity,
            Option<&GridFov>,
            Option<&SpatialFov>,
            Option<&GridFovState>,
            Option<&SpatialFovState>,
        ),
        Or<(With<GridFov>, With<SpatialFov>)>,
    >,
    mut stats: ResMut<FovStats>,
) {
    stats.dirty_viewers = 0;
    stats.recomputed_viewers = 0;
    stats.visible_cells_total = 0;
    stats.visible_targets_total = 0;
    stats.target_checks = 0;
    stats.occlusion_tests = 0;
    stats.last_recompute_micros = 0;

    for (entity, grid, spatial, grid_state, spatial_state) in &viewers {
        let mut entity_commands = commands.entity(entity);
        if grid.is_some() && grid_state.is_none() {
            entity_commands
                .insert(GridFovState::default())
                .insert(FovDirty);
        }
        if spatial.is_some() && spatial_state.is_none() {
            entity_commands
                .insert(SpatialFovState::default())
                .insert(FovDirty);
        }
    }
}

pub(crate) fn mark_viewers_dirty(
    mut commands: Commands,
    grid_map: Option<Res<GridOpacityMap>>,
    viewer_changes: Query<
        Entity,
        (
            Or<(
                Added<GridFov>,
                Changed<GridFov>,
                Added<SpatialFov>,
                Changed<SpatialFov>,
                Changed<Transform>,
                Changed<GlobalTransform>,
            )>,
            Or<(With<GridFov>, With<SpatialFov>)>,
        ),
    >,
    changed_targets: Query<
        (),
        Or<(
            Changed<FovTarget>,
            Changed<Transform>,
            Changed<GlobalTransform>,
        )>,
    >,
    changed_occluders: Query<
        (),
        Or<(
            Changed<FovOccluder>,
            Changed<Transform>,
            Changed<GlobalTransform>,
        )>,
    >,
    mut removed_targets: RemovedComponents<FovTarget>,
    mut removed_occluders: RemovedComponents<FovOccluder>,
    all_viewers: Query<Entity, Or<(With<GridFov>, With<SpatialFov>)>>,
    dirty_viewers: Query<Entity, With<FovDirty>>,
    mut stats: ResMut<FovStats>,
) {
    let mut pending_dirty = HashSet::new();
    pending_dirty.extend(dirty_viewers.iter());
    let global_dirty = grid_map.is_some_and(|map| map.is_changed())
        || !changed_targets.is_empty()
        || !changed_occluders.is_empty()
        || removed_targets.read().next().is_some()
        || removed_occluders.read().next().is_some();

    if global_dirty {
        for entity in &all_viewers {
            commands.entity(entity).insert(FovDirty);
            pending_dirty.insert(entity);
        }
    } else {
        for entity in &viewer_changes {
            commands.entity(entity).insert(FovDirty);
            pending_dirty.insert(entity);
        }
    }

    stats.dirty_viewers = pending_dirty.len();
}

pub(crate) fn recompute_viewers(
    mut commands: Commands,
    runtime_config: Res<FovRuntimeConfig>,
    grid_map: Option<Res<GridOpacityMap>>,
    dirty_viewers: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&GridFov>,
            Option<&SpatialFov>,
        ),
        With<FovDirty>,
    >,
    targets: Query<(Entity, &GlobalTransform, &FovTarget)>,
    occluders: Query<(&GlobalTransform, &FovOccluder)>,
    mut grid_states: Query<&mut GridFovState>,
    mut spatial_states: Query<&mut SpatialFovState>,
    mut stats: ResMut<FovStats>,
) {
    let start = Instant::now();

    let target_samples: Vec<_> = targets
        .iter()
        .filter(|(_, _, target)| target.enabled)
        .map(|(entity, transform, target)| {
            (
                entity,
                target.layers,
                target
                    .sample_points
                    .iter()
                    .copied()
                    .map(|offset| transform.transform_point(offset))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    let occluder_samples: Vec<_> = occluders
        .iter()
        .filter(|(_, occluder)| occluder.enabled)
        .map(|(transform, occluder)| {
            (
                occluder.layers,
                WorldOccluder {
                    shape: occluder.shape,
                    translation: transform.transform_point(occluder.local_offset),
                    rotation: transform.to_scale_rotation_translation().1,
                },
            )
        })
        .collect();

    let mut viewers: Vec<_> = dirty_viewers.iter().collect();
    viewers.sort_by_key(|(entity, _, _, _)| entity.index());

    for (entity, transform, grid_viewer, spatial_viewer) in viewers
        .into_iter()
        .take(runtime_config.max_viewers_per_frame)
    {
        if let Some(grid_viewer) = grid_viewer {
            update_grid_viewer(
                entity,
                transform,
                grid_viewer,
                grid_map.as_deref(),
                &mut grid_states,
                &mut stats,
            );
        }

        if let Some(spatial_viewer) = spatial_viewer {
            update_spatial_viewer(
                entity,
                transform,
                spatial_viewer,
                &target_samples,
                &occluder_samples,
                &mut spatial_states,
                &mut stats,
            );
        }

        commands.entity(entity).remove::<FovDirty>();
        stats.recomputed_viewers += 1;
    }

    stats.last_recompute_micros = start.elapsed().as_micros() as u64;
}

type DebugDrawState<'w, 's> = SystemState<(
    Res<'w, FovDebugSettings>,
    Option<Res<'w, GridOpacityMap>>,
    Gizmos<'w, 's, FovDebugGizmos>,
    Query<
        'w,
        's,
        (
            &'static GlobalTransform,
            &'static GridFov,
            &'static GridFovState,
        ),
    >,
    Query<
        'w,
        's,
        (
            &'static GlobalTransform,
            &'static SpatialFov,
            &'static SpatialFovState,
        ),
    >,
    Query<'w, 's, &'static GlobalTransform, With<FovTarget>>,
)>;

pub(crate) fn draw_debug(world: &mut World) {
    if !world.contains_resource::<GizmoStorage<FovDebugGizmos, ()>>() {
        return;
    }

    let mut state: DebugDrawState<'_, '_> = SystemState::new(world);
    let (debug, grid_map, mut gizmos, grid_viewers, spatial_viewers, targets) =
        state.get_mut(world);

    if debug.draw_grid_cells {
        if let Some(grid_map) = grid_map.as_deref() {
            for (transform, viewer, state) in &grid_viewers {
                let Some(origin_cell) = grid_map
                    .spec
                    .world_to_cell(transform.translation().truncate())
                else {
                    continue;
                };

                if debug.draw_view_shapes {
                    if let Some(center) = grid_map.spec.cell_to_world_center(origin_cell) {
                        gizmos.circle_2d(
                            center,
                            viewer.config.radius.max(0) as f32 * grid_map.spec.cell_size.x,
                            Color::from(css::GOLD),
                        );
                    }
                }

                for cell in state
                    .visible_now
                    .iter()
                    .take(debug.max_grid_cells_per_viewer)
                    .copied()
                {
                    if let Some((min, max)) = grid_map.spec.cell_to_world_rect(cell) {
                        let center = (min + max) * 0.5;
                        gizmos.rect_2d(
                            center,
                            grid_map.spec.cell_size,
                            Color::srgba(0.15, 0.85, 0.95, 0.9),
                        );
                    }
                }
            }
        }
    }

    for (transform, viewer, state) in &spatial_viewers {
        let origin = transform.transform_point(viewer.local_origin);
        let forward =
            transform.transform_point(viewer.local_origin + viewer.local_forward) - origin;

        if debug.draw_view_shapes {
            match viewer.dimension {
                SpatialDimension::Planar2d => {
                    let origin_2d = origin.truncate();
                    match viewer.shape {
                        SpatialShape::Radius { range } => {
                            gizmos.circle_2d(origin_2d, range, Color::from(css::ORANGE));
                        }
                        SpatialShape::Cone {
                            range,
                            half_angle_radians,
                        } => {
                            let rotation = forward.truncate().to_angle();
                            gizmos.arc_2d(
                                Isometry2d::new(origin_2d, Rot2::radians(rotation)),
                                half_angle_radians * 2.0,
                                range,
                                Color::from(css::ORANGE),
                            );
                            let left =
                                origin_2d + Vec2::from_angle(rotation - half_angle_radians) * range;
                            let right =
                                origin_2d + Vec2::from_angle(rotation + half_angle_radians) * range;
                            gizmos.line_2d(origin_2d, left, Color::from(css::ORANGE));
                            gizmos.line_2d(origin_2d, right, Color::from(css::ORANGE));
                        }
                    }
                }
                SpatialDimension::Volumetric3d => match viewer.shape {
                    SpatialShape::Radius { range } => {
                        gizmos.sphere(origin, range, Color::srgba(1.0, 0.5, 0.1, 0.15));
                    }
                    SpatialShape::Cone {
                        range,
                        half_angle_radians,
                    } => draw_cone_3d(
                        &mut gizmos,
                        origin,
                        forward,
                        range,
                        half_angle_radians,
                        Color::from(css::ORANGE),
                    ),
                },
            }
        }

        if debug.draw_occlusion_rays {
            for entity in &state.visible_now {
                if let Ok(target_transform) = targets.get(*entity) {
                    gizmos.line(
                        origin,
                        target_transform.translation(),
                        Color::from(css::LIME),
                    );
                }
            }
        }
    }
}

fn update_grid_viewer(
    entity: Entity,
    transform: &GlobalTransform,
    viewer: &GridFov,
    grid_map: Option<&GridOpacityMap>,
    grid_states: &mut Query<&mut GridFovState>,
    stats: &mut FovStats,
) {
    let Ok(mut state) = grid_states.get_mut(entity) else {
        return;
    };

    let Some(grid_map) = grid_map else {
        publish_grid_state(&mut state, Vec::new(), viewer.remember_seen_cells);
        return;
    };

    if !viewer.enabled {
        publish_grid_state(&mut state, Vec::new(), viewer.remember_seen_cells);
        return;
    }

    let Some(origin) = grid_map
        .spec
        .world_to_cell(transform.translation().truncate())
    else {
        publish_grid_state(&mut state, Vec::new(), viewer.remember_seen_cells);
        return;
    };

    let result = compute_grid_fov(grid_map.spec, origin, &viewer.config, |cell| {
        grid_map.is_opaque(cell)
    });
    stats.visible_cells_total += result.visible_cells.len();
    publish_grid_state(&mut state, result.visible_cells, viewer.remember_seen_cells);
}

fn update_spatial_viewer(
    entity: Entity,
    transform: &GlobalTransform,
    viewer: &SpatialFov,
    target_samples: &[(Entity, VisibilityLayerMask, Vec<Vec3>)],
    occluders: &[(VisibilityLayerMask, WorldOccluder)],
    spatial_states: &mut Query<&mut SpatialFovState>,
    stats: &mut FovStats,
) {
    let Ok(mut state) = spatial_states.get_mut(entity) else {
        return;
    };

    if !viewer.enabled {
        publish_spatial_state(&mut state, Vec::new(), viewer.remember_seen_targets);
        return;
    }

    let origin = transform.transform_point(viewer.local_origin);
    let forward = transform.transform_point(viewer.local_origin + viewer.local_forward) - origin;
    let query = SpatialVisibilityQuery {
        origin,
        forward,
        dimension: viewer.dimension,
        shape: viewer.shape,
        near_override: viewer.near_override,
    };

    let mut visible_now = Vec::new();
    let relevant_occluders: Vec<_> = occluders
        .iter()
        .filter(|(occluder_layers, _)| viewer.layers.overlaps(*occluder_layers))
        .map(|(_, occluder)| *occluder)
        .collect();

    for (target_entity, layers, samples) in target_samples {
        if !viewer.layers.overlaps(*layers) {
            continue;
        }

        let result = evaluate_visibility(&query, samples, |start, end| {
            occluded_by_any(start, end, viewer.dimension, &relevant_occluders)
        });
        stats.target_checks += result.checked_samples;
        stats.occlusion_tests += result.rays_cast;

        if result.visible {
            visible_now.push(*target_entity);
        }
    }

    visible_now.sort_by_key(|entity| entity.index());
    visible_now.dedup();
    stats.visible_targets_total += visible_now.len();
    publish_spatial_state(&mut state, visible_now, viewer.remember_seen_targets);
}

fn publish_grid_state(state: &mut GridFovState, visible_now: Vec<IVec2>, remember_seen: bool) {
    let old_visible: HashSet<_> = state.visible_now.iter().copied().collect();
    let new_visible: HashSet<_> = visible_now.iter().copied().collect();

    let mut entered: Vec<_> = visible_now
        .iter()
        .copied()
        .filter(|cell| !old_visible.contains(cell))
        .collect();
    let mut exited: Vec<_> = state
        .visible_now
        .iter()
        .copied()
        .filter(|cell| !new_visible.contains(cell))
        .collect();
    entered.sort_by_key(|cell| (cell.y, cell.x));
    exited.sort_by_key(|cell| (cell.y, cell.x));

    let mut explored = if remember_seen {
        let mut seen: HashSet<_> = state.explored.iter().copied().collect();
        seen.extend(visible_now.iter().copied());
        let mut seen_vec: Vec<_> = seen.into_iter().collect();
        seen_vec.sort_by_key(|cell| (cell.y, cell.x));
        seen_vec
    } else {
        visible_now.clone()
    };

    let changed = state.visible_now != visible_now || state.explored != explored;
    if !changed {
        return;
    }

    state.visible_now = visible_now;
    state.explored.clear();
    state.explored.append(&mut explored);
    state.entered = entered;
    state.exited = exited;
}

fn publish_spatial_state(
    state: &mut SpatialFovState,
    visible_now: Vec<Entity>,
    remember_seen: bool,
) {
    let old_visible: HashSet<_> = state.visible_now.iter().copied().collect();
    let new_visible: HashSet<_> = visible_now.iter().copied().collect();

    let mut entered: Vec<_> = visible_now
        .iter()
        .copied()
        .filter(|entity| !old_visible.contains(entity))
        .collect();
    let mut exited: Vec<_> = state
        .visible_now
        .iter()
        .copied()
        .filter(|entity| !new_visible.contains(entity))
        .collect();
    entered.sort_by_key(|entity| entity.index());
    exited.sort_by_key(|entity| entity.index());

    let mut remembered = if remember_seen {
        let mut seen: HashSet<_> = state.remembered.iter().copied().collect();
        seen.extend(visible_now.iter().copied());
        let mut seen_vec: Vec<_> = seen.into_iter().collect();
        seen_vec.sort_by_key(|entity| entity.index());
        seen_vec
    } else {
        visible_now.clone()
    };

    let changed = state.visible_now != visible_now || state.remembered != remembered;
    if !changed {
        return;
    }

    state.visible_now = visible_now;
    state.remembered.clear();
    state.remembered.append(&mut remembered);
    state.entered = entered;
    state.exited = exited;
}

fn draw_cone_3d(
    gizmos: &mut Gizmos<'_, '_, FovDebugGizmos>,
    origin: Vec3,
    forward: Vec3,
    range: f32,
    half_angle_radians: f32,
    color: Color,
) {
    let forward = forward.normalize_or_zero();
    if forward == Vec3::ZERO {
        gizmos.sphere(origin, range, color.with_alpha(0.15));
        return;
    }

    let base_center = origin + forward * range;
    let base_radius = range * half_angle_radians.tan();
    let orientation = Quat::from_rotation_arc(Vec3::Z, forward);
    gizmos.circle(
        Isometry3d::new(base_center, orientation),
        base_radius,
        color,
    );
    gizmos.arrow(origin, base_center, color);

    let tangent = if forward.cross(Vec3::Y).length_squared() > 0.0001 {
        forward.cross(Vec3::Y).normalize()
    } else {
        forward.cross(Vec3::X).normalize()
    };
    let bitangent = forward.cross(tangent).normalize();

    for direction in [tangent, -tangent, bitangent, -bitangent] {
        gizmos.line(origin, base_center + direction * base_radius, color);
    }
}

#[cfg(test)]
#[path = "systems_tests.rs"]
mod tests;
