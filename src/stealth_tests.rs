use std::time::Duration;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

use crate::{
    FovPlugin, FovStimulusSource, SpatialFov, SpatialStimulusConfig, StealthAwarenessConfig,
    StealthAwarenessLevel, StealthAwarenessPlugin, components::FovTarget,
};

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct DeactivateSchedule;

fn advance_time(app: &mut App, seconds: f32) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(seconds));
}

#[test]
fn stealth_plugin_maps_core_stimulus_into_alert_and_searching() {
    let mut app = App::new();
    app.insert_resource(Time::<()>::default());
    app.init_schedule(DeactivateSchedule);
    app.add_plugins(FovPlugin::new(Startup, DeactivateSchedule, Update));
    app.add_plugins(StealthAwarenessPlugin::default());

    let viewer = app
        .world_mut()
        .spawn((
            SpatialFov::cone_2d(10.0, 0.8).with_stimulus(SpatialStimulusConfig {
                gain_per_second: 2.0,
                loss_per_second: 2.0,
                forget_after_seconds: 0.4,
                ..default()
            }),
            StealthAwarenessConfig {
                alert_threshold: 0.5,
            },
            Transform::default(),
            GlobalTransform::IDENTITY,
        ))
        .id();

    let target = app
        .world_mut()
        .spawn((
            FovTarget::default(),
            FovStimulusSource {
                direct_visibility_scale: 1.4,
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
        .get::<crate::StealthAwarenessState>(viewer)
        .expect("stealth awareness state should exist");
    let entry = state
        .awareness_of(target)
        .expect("target should have stealth awareness entry");
    assert_eq!(entry.level, StealthAwarenessLevel::Alert);

    app.world_mut().entity_mut(target).insert((
        Transform::from_xyz(30.0, 0.0, 0.0),
        GlobalTransform::from_xyz(30.0, 0.0, 0.0),
    ));

    advance_time(&mut app, 0.3);
    app.update();
    let state = app
        .world()
        .get::<crate::StealthAwarenessState>(viewer)
        .expect("stealth awareness state should exist");
    let entry = state
        .awareness_of(target)
        .expect("target should remain remembered while signal decays");
    assert!(matches!(
        entry.level,
        StealthAwarenessLevel::Searching | StealthAwarenessLevel::Lost
    ));
}
