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
    awareness::{
        AwarenessLevel, SpatialAwarenessEntry, classify_awareness, should_forget_target,
        visibility_score_for_sample,
    },
    components::{
        FovDirty, FovOccluder, FovPerceptionModifiers, FovTarget, GridFov, GridFovState,
        SpatialFov, SpatialFovState,
    },
    debug::{FovDebugGizmos, FovDebugSettings},
    grid::GridOpacityMap,
    messages::{SpatialAwarenessChanged, SpatialTargetDetected, SpatialTargetLost},
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
            state.awareness.clear();
            state.remembered.clear();
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
    awareness_viewers: Query<(Entity, &SpatialFov)>,
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

    // Awareness-driven detection and forgetting are time-based, so these viewers
    // must continue recomputing even when no transforms or occluders changed.
    for (entity, viewer) in &awareness_viewers {
        if viewer.awareness.enabled {
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
        Option<&FovPerceptionModifiers>,
    )>,
    occluders: Query<(&GlobalTransform, &FovOccluder)>,
    mut grid_states: Query<&mut GridFovState>,
    mut spatial_states: Query<&mut SpatialFovState>,
    mut awareness_changed: MessageWriter<SpatialAwarenessChanged>,
    mut target_detected: MessageWriter<SpatialTargetDetected>,
    mut target_lost: MessageWriter<SpatialTargetLost>,
    mut stats: ResMut<FovStats>,
) {
    let start = Instant::now();

    let target_samples: Vec<_> = targets
        .iter()
        .filter(|(_, _, target, _)| target.enabled)
        .map(|(entity, transform, target, modifiers)| {
            (
                entity,
                target.layers,
                modifiers.copied().unwrap_or_default(),
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
                time.delta_secs(),
                &target_samples,
                &occluder_samples,
                &mut spatial_states,
                &mut awareness_changed,
                &mut target_detected,
                &mut target_lost,
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
    delta_seconds: f32,
    target_samples: &[(
        Entity,
        VisibilityLayerMask,
        FovPerceptionModifiers,
        Vec<Vec3>,
    )],
    occluders: &[(VisibilityLayerMask, WorldOccluder)],
    spatial_states: &mut Query<&mut SpatialFovState>,
    awareness_changed: &mut MessageWriter<SpatialAwarenessChanged>,
    target_detected: &mut MessageWriter<SpatialTargetDetected>,
    target_lost: &mut MessageWriter<SpatialTargetLost>,
    stats: &mut FovStats,
) {
    let Ok(mut state) = spatial_states.get_mut(entity) else {
        return;
    };

    if !viewer.enabled {
        publish_spatial_state(&mut state, Vec::new(), Vec::new(), Vec::new());
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
    let previous_awareness: HashMap<_, _> = state
        .awareness
        .iter()
        .cloned()
        .map(|entry| (entry.entity, entry))
        .collect();
    let mut next_awareness = Vec::new();
    let relevant_occluders: Vec<_> = occluders
        .iter()
        .filter(|(occluder_layers, _)| viewer.layers.overlaps(*occluder_layers))
        .map(|(_, occluder)| *occluder)
        .collect();

    for (target_entity, layers, modifiers, samples) in target_samples {
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

        if let Some(entry) = update_awareness_entry(
            *target_entity,
            viewer,
            &query,
            *modifiers,
            &result,
            delta_seconds,
            previous_awareness.get(target_entity).cloned(),
        ) {
            next_awareness.push(entry);
        }
    }

    visible_now.sort_by_key(|entity| entity.index());
    visible_now.dedup();
    next_awareness.sort_by_key(|entry| entry.entity.index());
    let remembered_now = if !viewer.awareness.enabled {
        if viewer.remember_seen_targets {
            let mut remembered = state.remembered.clone();
            remembered.extend(visible_now.iter().copied());
            remembered.sort_by_key(|entity| entity.index());
            remembered.dedup();
            remembered
        } else {
            visible_now.clone()
        }
    } else if viewer.remember_seen_targets {
        let mut remembered: Vec<_> = next_awareness
            .iter()
            .filter(|entry| entry.currently_visible || entry.last_seen_seconds_ago.is_some())
            .map(|entry| entry.entity)
            .collect();
        remembered.sort_by_key(|entity| entity.index());
        remembered.dedup();
        remembered
    } else {
        visible_now.clone()
    };

    emit_awareness_messages(
        entity,
        &previous_awareness,
        &next_awareness,
        awareness_changed,
        target_detected,
        target_lost,
    );
    stats.visible_targets_total += visible_now.len();
    publish_spatial_state(&mut state, visible_now, remembered_now, next_awareness);
}

fn update_awareness_entry(
    target_entity: Entity,
    viewer: &SpatialFov,
    query: &SpatialVisibilityQuery,
    modifiers: FovPerceptionModifiers,
    result: &crate::spatial::VisibilityTestResult,
    delta_seconds: f32,
    previous: Option<SpatialAwarenessEntry>,
) -> Option<SpatialAwarenessEntry> {
    if !viewer.awareness.enabled {
        return None;
    }

    let mut entry = previous.unwrap_or_else(|| SpatialAwarenessEntry::new(target_entity));
    let had_seen_target = entry.last_seen_seconds_ago.is_some() || result.visible;

    entry.currently_visible = result.visible;
    entry.visibility_score = 0.0;
    entry.focused = false;

    if result.visible {
        let sample = result.visible_sample.unwrap_or(query.origin);
        let score = visibility_score_for_sample(query, sample, &viewer.awareness);
        let gain = delta_seconds.max(0.0)
            * viewer.awareness.gain_per_second.max(0.0)
            * score.visibility_score
            * modifiers.light_exposure.max(0.0)
            * modifiers.awareness_gain_multiplier.max(0.0);
        entry.awareness = (entry.awareness + gain).clamp(0.0, viewer.awareness.max_awareness);
        entry.visibility_score = score.visibility_score;
        entry.focused = score.focused;
        entry.last_seen_seconds_ago = Some(0.0);
        entry.last_known_position = result.visible_sample;
    } else {
        if let Some(last_seen) = entry.last_seen_seconds_ago.as_mut() {
            *last_seen += delta_seconds.max(0.0);
        }

        let mut next_awareness = (entry.awareness
            - delta_seconds.max(0.0)
                * viewer.awareness.loss_per_second.max(0.0)
                * modifiers.awareness_loss_multiplier.max(0.0))
        .max(0.0);

        if result.in_range && !result.occluded && modifiers.noise_emission > 0.0 {
            next_awareness = (next_awareness
                + delta_seconds.max(0.0)
                    * viewer.awareness.noise_gain_per_second.max(0.0)
                    * modifiers.noise_emission.max(0.0))
            .clamp(0.0, viewer.awareness.max_awareness);
        }

        entry.awareness = next_awareness;
    }

    entry.level = classify_awareness(
        entry.awareness,
        entry.currently_visible,
        had_seen_target,
        viewer.awareness.forget_after_seconds,
        entry.last_seen_seconds_ago,
        viewer.awareness.alert_threshold,
    );

    if entry.level == AwarenessLevel::Unaware
        && should_forget_target(
            viewer.awareness.forget_after_seconds,
            entry.last_seen_seconds_ago,
        )
    {
        return None;
    }

    if entry.level == AwarenessLevel::Unaware
        && !entry.currently_visible
        && entry.last_seen_seconds_ago.is_none()
        && entry.awareness <= 0.0
    {
        return None;
    }

    Some(entry)
}

fn emit_awareness_messages(
    viewer: Entity,
    previous: &HashMap<Entity, SpatialAwarenessEntry>,
    next: &[SpatialAwarenessEntry],
    awareness_changed: &mut MessageWriter<SpatialAwarenessChanged>,
    target_detected: &mut MessageWriter<SpatialTargetDetected>,
    target_lost: &mut MessageWriter<SpatialTargetLost>,
) {
    let next_map: HashMap<_, _> = next
        .iter()
        .cloned()
        .map(|entry| (entry.entity, entry))
        .collect();

    for (target, entry) in &next_map {
        let previous_level = previous
            .get(target)
            .map(|previous| previous.level)
            .unwrap_or(AwarenessLevel::Unaware);
        if previous_level != entry.level {
            awareness_changed.write(SpatialAwarenessChanged {
                viewer,
                target: *target,
                previous_level,
                level: entry.level,
                awareness: entry.awareness,
                last_known_position: entry.last_known_position,
            });
        }

        if previous_level != AwarenessLevel::Alert && entry.level == AwarenessLevel::Alert {
            target_detected.write(SpatialTargetDetected {
                viewer,
                target: *target,
                awareness: entry.awareness,
                last_known_position: entry.last_known_position,
            });
        }
    }

    for (target, entry) in previous {
        let next_level = next_map
            .get(target)
            .map(|next| next.level)
            .unwrap_or(AwarenessLevel::Unaware);
        if entry.level == AwarenessLevel::Alert && next_level != AwarenessLevel::Alert {
            let next_entry = next_map.get(target);
            target_lost.write(SpatialTargetLost {
                viewer,
                target: *target,
                awareness: next_entry.map(|next| next.awareness).unwrap_or(0.0),
                last_known_position: next_entry
                    .and_then(|next| next.last_known_position)
                    .or(entry.last_known_position),
            });
        }
    }
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
    remembered_now: Vec<Entity>,
    awareness_entries: Vec<SpatialAwarenessEntry>,
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

    let changed = state.visible_now != visible_now
        || state.remembered != remembered_now
        || state.awareness != awareness_entries;
    if !changed {
        return;
    }

    state.visible_now = visible_now;
    state.remembered = remembered_now;
    state.entered = entered;
    state.exited = exited;
    state.awareness = awareness_entries;
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
        // First edge in green to break symmetry and show rotation
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

#[cfg(test)]
#[path = "systems_tests.rs"]
mod tests;
