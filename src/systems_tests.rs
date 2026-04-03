use std::time::Duration;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

use super::*;
use crate::{
    AwarenessLevel, FovPlugin, SpatialAwarenessConfig,
    components::{FovOccluder, FovPerceptionModifiers, FovTarget, GridFov, SpatialFov},
    grid::{GridCornerPolicy, GridFovBackend, GridFovConfig, GridMapSpec, GridOpacityMap},
    spatial::{OccluderShape, VisibilityLayer, VisibilityLayerMask},
};

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct DeactivateSchedule;

fn advance_time(app: &mut App, seconds: f32) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(seconds));
}

#[test]
fn moving_grid_viewer_marks_dirty_and_recomputes() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(Time::<()>::default());
    app.init_schedule(DeactivateSchedule);
    app.insert_resource(GridOpacityMap::new(GridMapSpec {
        origin: Vec2::ZERO,
        dimensions: UVec2::new(10, 10),
        cell_size: Vec2::ONE,
    }));
    app.add_plugins(FovPlugin::new(Startup, DeactivateSchedule, Update));

    let viewer = app
        .world_mut()
        .spawn((
            GridFov::new(3),
            Transform::from_xyz(2.5, 2.5, 0.0),
            GlobalTransform::from_xyz(2.5, 2.5, 0.0),
        ))
        .id();

    app.update();
    let before = app
        .world()
        .get::<GridFovState>(viewer)
        .expect("grid state should exist")
        .visible_now
        .clone();
    assert!(!before.is_empty());

    app.world_mut().entity_mut(viewer).insert((
        Transform::from_xyz(5.5, 2.5, 0.0),
        GlobalTransform::from_xyz(5.5, 2.5, 0.0),
    ));
    app.update();

    let after = app
        .world()
        .get::<GridFovState>(viewer)
        .expect("grid state should exist")
        .clone();
    assert_ne!(before, after.visible_now);
    assert!(!after.entered.is_empty());
    assert!(!after.exited.is_empty());
}

#[test]
fn viewer_budget_staggers_updates() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(Time::<()>::default());
    app.insert_resource(FovRuntimeConfig {
        max_viewers_per_frame: 1,
    });
    app.insert_resource(GridOpacityMap::new(GridMapSpec {
        origin: Vec2::ZERO,
        dimensions: UVec2::new(10, 10),
        cell_size: Vec2::ONE,
    }));
    app.add_plugins(FovPlugin::default());

    let viewer_a = app
        .world_mut()
        .spawn((
            GridFov::new(3),
            Transform::from_xyz(1.5, 1.5, 0.0),
            GlobalTransform::from_xyz(1.5, 1.5, 0.0),
        ))
        .id();
    let viewer_b = app
        .world_mut()
        .spawn((
            GridFov::new(3),
            Transform::from_xyz(7.5, 7.5, 0.0),
            GlobalTransform::from_xyz(7.5, 7.5, 0.0),
        ))
        .id();

    app.update();
    assert!(app.world().get::<GridFovState>(viewer_a).is_some());
    assert!(app.world().get::<GridFovState>(viewer_b).is_some());
    assert_eq!(app.world().resource::<FovStats>().dirty_viewers, 2);
    assert_eq!(app.world().resource::<FovStats>().recomputed_viewers, 1);

    app.update();
    assert_eq!(app.world().resource::<FovStats>().dirty_viewers, 1);
    assert_eq!(app.world().resource::<FovStats>().recomputed_viewers, 1);
}

#[test]
fn spatial_visibility_filters_targets_by_layer_and_occlusion() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(Time::<()>::default());
    app.add_plugins(FovPlugin::default());

    let viewer = app
        .world_mut()
        .spawn((
            SpatialFov::cone_2d(8.0, std::f32::consts::FRAC_PI_4)
                .with_layers(VisibilityLayerMask::from_layer(VisibilityLayer::ZERO)),
            Transform::from_xyz(0.0, 0.0, 0.0),
            GlobalTransform::IDENTITY,
        ))
        .id();

    let visible_target = app
        .world_mut()
        .spawn((
            FovTarget::default()
                .with_layers(VisibilityLayerMask::from_layer(VisibilityLayer::ZERO)),
            Transform::from_xyz(4.0, 0.0, 0.0),
            GlobalTransform::from_xyz(4.0, 0.0, 0.0),
        ))
        .id();

    let hidden_layer_target = app
        .world_mut()
        .spawn((
            FovTarget::default().with_layers(VisibilityLayerMask::from_layer(VisibilityLayer(2))),
            Transform::from_xyz(3.0, 0.0, 0.0),
            GlobalTransform::from_xyz(3.0, 0.0, 0.0),
        ))
        .id();

    app.world_mut().spawn((
        FovOccluder::new(OccluderShape::Rect2d {
            half_extents: Vec2::new(0.5, 1.2),
        })
        .with_layers(VisibilityLayerMask::from_layer(VisibilityLayer::ZERO)),
        Transform::from_xyz(6.0, 0.0, 0.0),
        GlobalTransform::from_xyz(6.0, 0.0, 0.0),
    ));

    let blocked_target = app
        .world_mut()
        .spawn((
            FovTarget::default()
                .with_layers(VisibilityLayerMask::from_layer(VisibilityLayer::ZERO)),
            Transform::from_xyz(8.0, 0.0, 0.0),
            GlobalTransform::from_xyz(8.0, 0.0, 0.0),
        ))
        .id();

    app.update();

    let state = app
        .world()
        .get::<SpatialFovState>(viewer)
        .expect("spatial state should exist");
    assert!(state.visible_now.contains(&visible_target));
    assert!(!state.visible_now.contains(&hidden_layer_target));
    assert!(!state.visible_now.contains(&blocked_target));
}

#[test]
fn no_render_app_debug_toggle_does_not_panic() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(Time::<()>::default());
    app.insert_resource(FovDebugSettings {
        enabled: true,
        ..default()
    });
    app.insert_resource(GridOpacityMap::new(GridMapSpec {
        origin: Vec2::ZERO,
        dimensions: UVec2::new(8, 8),
        cell_size: Vec2::ONE,
    }));
    app.add_plugins(FovPlugin::default());
    app.world_mut().spawn((
        GridFov::new(2).with_config(GridFovConfig {
            radius: 2,
            backend: GridFovBackend::RecursiveShadowcasting,
            corner_policy: GridCornerPolicy::BlockIfBothAdjacentWalls,
            reveal_blockers: true,
        }),
        Transform::from_xyz(2.5, 2.5, 0.0),
        GlobalTransform::from_xyz(2.5, 2.5, 0.0),
    ));

    app.update();
}

#[test]
fn awareness_progresses_from_suspicious_to_alert_and_then_forgets() {
    let mut app = App::new();
    app.insert_resource(Time::<()>::default());
    app.init_schedule(DeactivateSchedule);
    app.add_plugins(FovPlugin::new(Startup, DeactivateSchedule, Update));

    let viewer = app
        .world_mut()
        .spawn((
            SpatialFov::cone_2d(10.0, 0.8).with_awareness(SpatialAwarenessConfig {
                gain_per_second: 2.0,
                loss_per_second: 2.0,
                alert_threshold: 0.5,
                forget_after_seconds: 0.4,
                ..default()
            }),
            Transform::default(),
            GlobalTransform::IDENTITY,
        ))
        .id();

    let target = app
        .world_mut()
        .spawn((
            FovTarget::default(),
            FovPerceptionModifiers {
                light_exposure: 1.4,
                ..default()
            },
            Transform::from_xyz(3.0, 0.0, 0.0),
            GlobalTransform::from_xyz(3.0, 0.0, 0.0),
        ))
        .id();

    advance_time(&mut app, 0.25);
    app.update();
    let state = app
        .world()
        .get::<SpatialFovState>(viewer)
        .expect("spatial state should exist");
    let awareness = state
        .awareness_of(target)
        .expect("target should have awareness entry");
    assert_eq!(awareness.level, AwarenessLevel::Alert);
    assert!(awareness.awareness >= 0.5);

    app.world_mut().entity_mut(target).insert((
        Transform::from_xyz(30.0, 0.0, 0.0),
        GlobalTransform::from_xyz(30.0, 0.0, 0.0),
    ));

    advance_time(&mut app, 0.3);
    app.update();
    let state = app
        .world()
        .get::<SpatialFovState>(viewer)
        .expect("spatial state should exist");
    let awareness = state
        .awareness_of(target)
        .expect("target should still be remembered while searching");
    assert!(matches!(
        awareness.level,
        AwarenessLevel::Searching | AwarenessLevel::Lost
    ));

    advance_time(&mut app, 0.5);
    app.update();
    let state = app
        .world()
        .get::<SpatialFovState>(viewer)
        .expect("spatial state should exist");
    assert!(state.awareness_of(target).is_none());
}
