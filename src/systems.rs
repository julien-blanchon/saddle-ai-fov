use std::collections::{HashMap, HashSet};

use bevy::platform::time::Instant;
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
        FovDirty, FovOccluder, FovStimulusSource, FovTarget, GridFov, GridFovState, SpatialFov,
        SpatialFovState,
    },
    debug::{FovDebugGizmos, FovDebugSettings},
    grid::GridOpacityMap,
    messages::{GridVisibilityChanged, SpatialStimulusChanged, SpatialVisibilityChanged},
    resources::FovStats,
    spatial::{
        OccluderShape, SpatialDimension, SpatialShape, SpatialVisibilityQuery, VisibilityLayerMask,
        WorldOccluder, evaluate_visibility, occluded_by_any,
    },
    stimulus::{SpatialStimulusEntry, should_forget_target, visibility_score_for_sample},
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
            state.entered.clear();
            state.exited = state.visible_now.clone();
            state.visible_now.clear();
        }

        if let Ok(mut state) = spatial_states.get_mut(entity) {
            state.entered.clear();
            state.exited = state.visible_now.clone();
            state.visible_now.clear();
            state.remembered.clear();
            state.stimuli.clear();
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
            Changed<FovStimulusSource>,
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
    mut removed_stimulus_sources: RemovedComponents<FovStimulusSource>,
    mut removed_occluders: RemovedComponents<FovOccluder>,
    all_viewers: Query<Entity, Or<(With<GridFov>, With<SpatialFov>)>>,
    stimulus_viewers: Query<(Entity, &SpatialFov)>,
    dirty_viewers: Query<Entity, With<FovDirty>>,
    mut stats: ResMut<FovStats>,
) {
    let mut pending_dirty = HashSet::new();
    pending_dirty.extend(dirty_viewers.iter());
    let global_dirty = grid_map.is_some_and(|map| map.is_changed())
        || !changed_targets.is_empty()
        || !changed_occluders.is_empty()
        || removed_targets.read().next().is_some()
        || removed_stimulus_sources.read().next().is_some()
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

    // Numeric stimulus accumulation and forgetting are time-based.
    for (entity, viewer) in &stimulus_viewers {
        if viewer.stimulus.enabled {
            commands.entity(entity).insert(FovDirty);
            pending_dirty.insert(entity);
        }
    }

    stats.dirty_viewers = pending_dirty.len();
}

pub(crate) fn recompute_viewers(
    time: Res<Time>,
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
    targets: Query<(
        Entity,
        &GlobalTransform,
        &FovTarget,
        Option<&FovStimulusSource>,
    )>,
    occluders: Query<(&GlobalTransform, &FovOccluder)>,
    mut grid_states: Query<&mut GridFovState>,
    mut spatial_states: Query<&mut SpatialFovState>,
    mut grid_visibility_changed: MessageWriter<GridVisibilityChanged>,
    mut spatial_visibility_changed: MessageWriter<SpatialVisibilityChanged>,
    mut stimulus_changed: MessageWriter<SpatialStimulusChanged>,
    mut stats: ResMut<FovStats>,
) {
    let start = Instant::now();

    let target_samples: Vec<_> = targets
        .iter()
        .filter(|(_, _, target, _)| target.enabled)
        .map(|(entity, transform, target, stimulus_source)| {
            (
                entity,
                target.layers,
                stimulus_source.copied().unwrap_or_default(),
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
            if let Ok(state) = grid_states.get(entity) {
                if !state.entered.is_empty() || !state.exited.is_empty() {
                    grid_visibility_changed.write(GridVisibilityChanged {
                        viewer: entity,
                        entered: state.entered.clone(),
                        exited: state.exited.clone(),
                    });
                }
            }
        }

        if let Some(spatial_viewer) = spatial_viewer {
            update_spatial_viewer(
                entity,
                transform,
                spatial_viewer,
                time.delta_secs(),
                &target_samples,
                &occluder_samples,
                &mut spatial_states,
                &mut spatial_visibility_changed,
                &mut stimulus_changed,
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
    Query<'w, 's, (Entity, &'static GlobalTransform), With<FovTarget>>,
    Query<'w, 's, (&'static GlobalTransform, &'static FovOccluder)>,
)>;

pub(crate) fn draw_debug(world: &mut World) {
    if !world.contains_resource::<GizmoStorage<FovDebugGizmos, ()>>() {
        return;
    }

    let mut state: DebugDrawState<'_, '_> = SystemState::new(world);
    let (debug, grid_map, mut gizmos, grid_viewers, spatial_viewers, targets, occluders) =
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
                            if debug.draw_filled_shapes {
                                let fill = Color::srgba(1.0, 0.55, 0.15, 0.03);
                                for frac in [0.33, 0.66] {
                                    gizmos.circle_2d(origin_2d, range * frac, fill);
                                }
                            }
                            gizmos.circle_2d(origin_2d, range, Color::from(css::ORANGE));
                            if viewer.near_override > 0.0 {
                                gizmos.circle_2d(
                                    origin_2d,
                                    viewer.near_override,
                                    Color::srgba(1.0, 0.85, 0.2, 0.35),
                                );
                            }
                        }
                        SpatialShape::Cone {
                            range,
                            half_angle_radians,
                        } => {
                            let rotation = forward.truncate().to_angle();

                            if debug.draw_filled_shapes {
                                let fill = Color::srgba(1.0, 0.55, 0.15, 0.06);
                                let steps = 16u32;
                                for i in 0..=steps {
                                    let t = i as f32 / steps as f32;
                                    let a = rotation - half_angle_radians
                                        + t * 2.0 * half_angle_radians;
                                    gizmos.line_2d(
                                        origin_2d,
                                        origin_2d + Vec2::from_angle(a) * range,
                                        fill,
                                    );
                                }
                                for frac in [0.33, 0.66] {
                                    gizmos.arc_2d(
                                        Isometry2d::new(origin_2d, Rot2::radians(rotation)),
                                        half_angle_radians * 2.0,
                                        range * frac,
                                        fill,
                                    );
                                }
                            }

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

                            if viewer.near_override > 0.0 {
                                gizmos.circle_2d(
                                    origin_2d,
                                    viewer.near_override,
                                    Color::srgba(1.0, 0.85, 0.2, 0.35),
                                );
                            }
                        }
                        SpatialShape::Rect {
                            depth, half_width, ..
                        } => {
                            let fwd_2d = forward.truncate().normalize_or_zero();
                            let right_2d = Vec2::new(-fwd_2d.y, fwd_2d.x);
                            let far = origin_2d + fwd_2d * depth;
                            let corners = [
                                origin_2d - right_2d * half_width,
                                origin_2d + right_2d * half_width,
                                far + right_2d * half_width,
                                far - right_2d * half_width,
                            ];
                            let outline_color = Color::from(css::ORANGE);
                            for i in 0..4 {
                                gizmos.line_2d(corners[i], corners[(i + 1) % 4], outline_color);
                            }
                            gizmos.line_2d(origin_2d, far, outline_color);

                            if debug.draw_filled_shapes {
                                let fill = Color::srgba(1.0, 0.55, 0.15, 0.06);
                                for frac in [0.33, 0.66] {
                                    let mid = origin_2d + fwd_2d * depth * frac;
                                    gizmos.line_2d(
                                        mid - right_2d * half_width,
                                        mid + right_2d * half_width,
                                        fill,
                                    );
                                }
                            }

                            if viewer.near_override > 0.0 {
                                gizmos.circle_2d(
                                    origin_2d,
                                    viewer.near_override,
                                    Color::srgba(1.0, 0.85, 0.2, 0.35),
                                );
                            }
                        }
                    }
                }
                SpatialDimension::Volumetric3d => match viewer.shape {
                    SpatialShape::Radius { range } => {
                        gizmos.sphere(origin, range, Color::srgba(1.0, 0.5, 0.1, 0.15));
                        if viewer.near_override > 0.0 {
                            gizmos.sphere(
                                origin,
                                viewer.near_override,
                                Color::srgba(1.0, 0.85, 0.2, 0.15),
                            );
                        }
                    }
                    SpatialShape::Cone {
                        range,
                        half_angle_radians,
                    } => {
                        draw_cone_3d(
                            &mut gizmos,
                            origin,
                            forward,
                            range,
                            half_angle_radians,
                            Color::from(css::ORANGE),
                            debug.draw_filled_shapes,
                        );
                        if viewer.near_override > 0.0 {
                            gizmos.sphere(
                                origin,
                                viewer.near_override,
                                Color::srgba(1.0, 0.85, 0.2, 0.15),
                            );
                        }
                    }
                    SpatialShape::Rect {
                        depth,
                        half_width,
                        half_height,
                    } => {
                        draw_rect_3d(
                            &mut gizmos,
                            origin,
                            forward,
                            depth,
                            half_width,
                            half_height,
                            Color::from(css::ORANGE),
                            debug.draw_filled_shapes,
                        );
                        if viewer.near_override > 0.0 {
                            gizmos.sphere(
                                origin,
                                viewer.near_override,
                                Color::srgba(1.0, 0.85, 0.2, 0.15),
                            );
                        }
                    }
                },
            }
        }

        if debug.draw_occlusion_rays || debug.draw_blocked_rays {
            for (target_entity, target_transform) in &targets {
                let target_pos = target_transform.translation();
                if state.visible_now.contains(&target_entity) {
                    if debug.draw_occlusion_rays {
                        gizmos.line(origin, target_pos, Color::from(css::LIME));
                    }
                } else if debug.draw_blocked_rays {
                    let dist = match viewer.dimension {
                        SpatialDimension::Planar2d => {
                            origin.truncate().distance(target_pos.truncate())
                        }
                        SpatialDimension::Volumetric3d => origin.distance(target_pos),
                    };
                    if dist <= viewer.shape.range() {
                        gizmos.line(origin, target_pos, Color::srgba(0.85, 0.20, 0.15, 0.35));
                    }
                }
            }
        }
    }

    if debug.draw_occluder_shapes {
        for (occ_transform, occluder) in &occluders {
            if !occluder.enabled {
                continue;
            }
            let pos = occ_transform.transform_point(occluder.local_offset);
            let rot = occ_transform.to_scale_rotation_translation().1;
            let outline = Color::srgba(0.65, 0.30, 0.80, 0.55);
            match occluder.shape {
                OccluderShape::Disc2d { radius } => {
                    gizmos.circle_2d(pos.truncate(), radius, outline);
                }
                OccluderShape::Rect2d { half_extents } => {
                    let fwd = rot * Vec3::X;
                    let right2d = Vec2::new(-fwd.y, fwd.x);
                    let fwd2d = Vec2::new(fwd.x, fwd.y);
                    let center = pos.truncate();
                    let corners = [
                        center + fwd2d * half_extents.y + right2d * half_extents.x,
                        center + fwd2d * half_extents.y - right2d * half_extents.x,
                        center - fwd2d * half_extents.y - right2d * half_extents.x,
                        center - fwd2d * half_extents.y + right2d * half_extents.x,
                    ];
                    for i in 0..4 {
                        gizmos.line_2d(corners[i], corners[(i + 1) % 4], outline);
                    }
                }
                OccluderShape::Sphere { radius } => {
                    gizmos.sphere(pos, radius, outline);
                }
                OccluderShape::Box { half_extents } => {
                    let he = half_extents;
                    let local_corners = [
                        Vec3::new(-he.x, -he.y, -he.z),
                        Vec3::new(he.x, -he.y, -he.z),
                        Vec3::new(he.x, he.y, -he.z),
                        Vec3::new(-he.x, he.y, -he.z),
                        Vec3::new(-he.x, -he.y, he.z),
                        Vec3::new(he.x, -he.y, he.z),
                        Vec3::new(he.x, he.y, he.z),
                        Vec3::new(-he.x, he.y, he.z),
                    ];
                    let world: Vec<Vec3> = local_corners.iter().map(|c| pos + rot * *c).collect();
                    for i in 0..4 {
                        gizmos.line(world[i], world[(i + 1) % 4], outline);
                    }
                    for i in 4..8 {
                        gizmos.line(world[i], world[4 + (i - 4 + 1) % 4], outline);
                    }
                    for i in 0..4 {
                        gizmos.line(world[i], world[i + 4], outline);
                    }
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
    delta_seconds: f32,
    target_samples: &[(Entity, VisibilityLayerMask, FovStimulusSource, Vec<Vec3>)],
    occluders: &[(VisibilityLayerMask, WorldOccluder)],
    spatial_states: &mut Query<&mut SpatialFovState>,
    spatial_visibility_changed: &mut MessageWriter<SpatialVisibilityChanged>,
    stimulus_changed: &mut MessageWriter<SpatialStimulusChanged>,
    stats: &mut FovStats,
) {
    let Ok(mut state) = spatial_states.get_mut(entity) else {
        return;
    };

    let previous_stimuli: HashMap<_, _> = state
        .stimuli
        .iter()
        .cloned()
        .map(|entry| (entry.entity, entry))
        .collect();

    if !viewer.enabled {
        publish_spatial_state(&mut state, Vec::new(), Vec::new(), Vec::new());
        if !state.entered.is_empty() || !state.exited.is_empty() {
            spatial_visibility_changed.write(SpatialVisibilityChanged {
                viewer: entity,
                entered: state.entered.clone(),
                exited: state.exited.clone(),
            });
        }
        emit_stimulus_messages(entity, &previous_stimuli, &state.stimuli, stimulus_changed);
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
    let mut next_stimuli = Vec::new();
    let relevant_occluders: Vec<_> = occluders
        .iter()
        .filter(|(occluder_layers, _)| viewer.layers.overlaps(*occluder_layers))
        .map(|(_, occluder)| *occluder)
        .collect();

    for (target_entity, layers, source, samples) in target_samples {
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

        if let Some(entry) = update_stimulus_entry(
            *target_entity,
            viewer,
            &query,
            *source,
            &result,
            delta_seconds,
            previous_stimuli.get(target_entity).cloned(),
        ) {
            next_stimuli.push(entry);
        }
    }

    visible_now.sort_by_key(|entity| entity.index());
    visible_now.dedup();
    next_stimuli.sort_by_key(|entry| entry.entity.index());

    let remembered_now = if viewer.remember_seen_targets {
        let mut remembered = state.remembered.clone();
        remembered.extend(visible_now.iter().copied());
        remembered.sort_by_key(|entity| entity.index());
        remembered.dedup();
        remembered
    } else {
        visible_now.clone()
    };

    stats.visible_targets_total += visible_now.len();
    publish_spatial_state(&mut state, visible_now, remembered_now, next_stimuli);

    if !state.entered.is_empty() || !state.exited.is_empty() {
        spatial_visibility_changed.write(SpatialVisibilityChanged {
            viewer: entity,
            entered: state.entered.clone(),
            exited: state.exited.clone(),
        });
    }
    emit_stimulus_messages(entity, &previous_stimuli, &state.stimuli, stimulus_changed);
}

fn update_stimulus_entry(
    target_entity: Entity,
    viewer: &SpatialFov,
    query: &SpatialVisibilityQuery,
    source: FovStimulusSource,
    result: &crate::spatial::VisibilityTestResult,
    delta_seconds: f32,
    previous: Option<SpatialStimulusEntry>,
) -> Option<SpatialStimulusEntry> {
    if !viewer.stimulus.enabled {
        return None;
    }

    let delta_seconds = delta_seconds.max(0.0);
    let mut entry = previous.unwrap_or_else(|| SpatialStimulusEntry::new(target_entity));

    entry.currently_visible = result.visible;
    entry.in_range = result.in_range;
    entry.inside_shape = result.inside_shape;
    entry.occluded = result.occluded;
    entry.visibility_score = 0.0;
    entry.indirect_signal = 0.0;
    entry.focused = false;

    if result.visible {
        let sample = result.visible_sample.unwrap_or(query.origin);
        let score = visibility_score_for_sample(query, sample, &viewer.stimulus);
        let gain = delta_seconds
            * viewer.stimulus.gain_per_second.max(0.0)
            * score.visibility_score
            * source.direct_visibility_scale.max(0.0)
            * source.signal_gain_multiplier.max(0.0);
        entry.signal = (entry.signal + gain).clamp(0.0, viewer.stimulus.max_signal);
        entry.visibility_score = score.visibility_score;
        entry.focused = score.focused;
        entry.last_seen_seconds_ago = Some(0.0);
        entry.last_known_position = result.visible_sample;
    } else {
        if let Some(last_seen) = entry.last_seen_seconds_ago.as_mut() {
            *last_seen += delta_seconds;
        }

        let mut next_signal = (entry.signal
            - delta_seconds
                * viewer.stimulus.loss_per_second.max(0.0)
                * source.signal_loss_multiplier.max(0.0))
        .max(0.0);

        let indirect_signal = if result.in_range && !result.occluded {
            delta_seconds
                * viewer.stimulus.indirect_gain_per_second.max(0.0)
                * source.indirect_signal.max(0.0)
                * source.signal_gain_multiplier.max(0.0)
        } else {
            0.0
        };

        next_signal = (next_signal + indirect_signal).clamp(0.0, viewer.stimulus.max_signal);
        entry.signal = next_signal;
        entry.indirect_signal = indirect_signal;
    }

    if should_forget_target(
        viewer.stimulus.forget_after_seconds,
        entry.last_seen_seconds_ago,
    ) && !entry.currently_visible
        && entry.signal <= 0.0
    {
        return None;
    }

    if !entry.currently_visible && entry.last_seen_seconds_ago.is_none() && entry.signal <= 0.0 {
        return None;
    }

    Some(entry)
}

fn emit_stimulus_messages(
    viewer: Entity,
    previous: &HashMap<Entity, SpatialStimulusEntry>,
    next: &[SpatialStimulusEntry],
    stimulus_changed: &mut MessageWriter<SpatialStimulusChanged>,
) {
    let next_map: HashMap<_, _> = next
        .iter()
        .cloned()
        .map(|entry| (entry.entity, entry))
        .collect();

    for (target, entry) in &next_map {
        if previous.get(target) == Some(entry) {
            continue;
        }

        stimulus_changed.write(SpatialStimulusChanged {
            viewer,
            target: *target,
            previous_signal: previous.get(target).map_or(0.0, |previous| previous.signal),
            signal: entry.signal,
            visibility_score: entry.visibility_score,
            indirect_signal: entry.indirect_signal,
            currently_visible: entry.currently_visible,
            in_range: entry.in_range,
            inside_shape: entry.inside_shape,
            occluded: entry.occluded,
            last_known_position: entry.last_known_position,
        });
    }

    for (target, entry) in previous {
        if next_map.contains_key(target) {
            continue;
        }

        stimulus_changed.write(SpatialStimulusChanged {
            viewer,
            target: *target,
            previous_signal: entry.signal,
            signal: 0.0,
            visibility_score: 0.0,
            indirect_signal: 0.0,
            currently_visible: false,
            in_range: false,
            inside_shape: false,
            occluded: false,
            last_known_position: entry.last_known_position,
        });
    }
}

fn publish_grid_state(
    state: &mut GridFovState,
    visible_now: Vec<IVec2>,
    remember_seen: bool,
) -> bool {
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
        state.entered.clear();
        state.exited.clear();
        return false;
    }

    state.visible_now = visible_now;
    state.explored.clear();
    state.explored.append(&mut explored);
    state.entered = entered;
    state.exited = exited;
    true
}

fn publish_spatial_state(
    state: &mut SpatialFovState,
    visible_now: Vec<Entity>,
    remembered_now: Vec<Entity>,
    stimuli: Vec<SpatialStimulusEntry>,
) -> bool {
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

    let changed = state.visible_now != visible_now
        || state.remembered != remembered_now
        || state.stimuli != stimuli;
    if !changed {
        state.entered.clear();
        state.exited.clear();
        return false;
    }

    state.visible_now = visible_now;
    state.remembered = remembered_now;
    state.entered = entered;
    state.exited = exited;
    state.stimuli = stimuli;
    true
}

fn draw_cone_3d(
    gizmos: &mut Gizmos<'_, '_, FovDebugGizmos>,
    origin: Vec3,
    forward: Vec3,
    range: f32,
    half_angle_radians: f32,
    color: Color,
    filled: bool,
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

    let edge_count = 8;
    for i in 0..edge_count {
        let angle = std::f32::consts::TAU * i as f32 / edge_count as f32;
        let dir = tangent * angle.cos() + bitangent * angle.sin();
        let edge_color = if i == 0 {
            Color::from(css::LIMEGREEN)
        } else {
            color
        };
        gizmos.line(origin, base_center + dir * base_radius, edge_color);
    }

    if filled {
        let fill = color.with_alpha(0.06);
        for frac in [0.33, 0.66] {
            let mid = origin + forward * range * frac;
            let r = range * frac * half_angle_radians.tan();
            gizmos.circle(Isometry3d::new(mid, orientation), r, fill);
        }
    }
}

fn draw_rect_3d(
    gizmos: &mut Gizmos<'_, '_, FovDebugGizmos>,
    origin: Vec3,
    forward: Vec3,
    depth: f32,
    half_width: f32,
    half_height: f32,
    color: Color,
    filled: bool,
) {
    let forward = forward.normalize_or_zero();
    if forward == Vec3::ZERO {
        gizmos.sphere(origin, depth, color.with_alpha(0.15));
        return;
    }

    let up_hint = if forward.cross(Vec3::Y).length_squared() > 0.0001 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let right = forward.cross(up_hint).normalize();
    let up = right.cross(forward).normalize();

    let far = origin + forward * depth;
    let n0 = origin - right * half_width - up * half_height;
    let n1 = origin + right * half_width - up * half_height;
    let n2 = origin + right * half_width + up * half_height;
    let n3 = origin - right * half_width + up * half_height;
    let f0 = far - right * half_width - up * half_height;
    let f1 = far + right * half_width - up * half_height;
    let f2 = far + right * half_width + up * half_height;
    let f3 = far - right * half_width + up * half_height;

    gizmos.line(n0, n1, color);
    gizmos.line(n1, n2, color);
    gizmos.line(n2, n3, color);
    gizmos.line(n3, n0, color);
    gizmos.line(f0, f1, color);
    gizmos.line(f1, f2, color);
    gizmos.line(f2, f3, color);
    gizmos.line(f3, f0, color);
    gizmos.line(n0, f0, Color::from(css::LIMEGREEN));
    gizmos.line(n1, f1, color);
    gizmos.line(n2, f2, color);
    gizmos.line(n3, f3, color);
    gizmos.arrow(origin, far, color);

    if filled {
        let fill = color.with_alpha(0.06);
        for frac in [0.33, 0.66] {
            let mid = origin + forward * depth * frac;
            let m0 = mid - right * half_width - up * half_height;
            let m1 = mid + right * half_width - up * half_height;
            let m2 = mid + right * half_width + up * half_height;
            let m3 = mid - right * half_width + up * half_height;
            gizmos.line(m0, m1, fill);
            gizmos.line(m1, m2, fill);
            gizmos.line(m2, m3, fill);
            gizmos.line(m3, m0, fill);
        }
    }
}

#[cfg(test)]
#[path = "systems_tests.rs"]
mod tests;
