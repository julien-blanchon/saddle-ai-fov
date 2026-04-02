use bevy::prelude::*;
use saddle_bevy_e2e::{action::Action, actions::assertions, scenario::Scenario};

use crate::{set_grid_viewer_cell, set_guard_angle, set_pause_motion};

pub fn list_scenarios() -> Vec<&'static str> {
    vec![
        "smoke_launch",
        "fov_smoke",
        "fov_grid_memory",
        "fov_cone_occlusion",
    ]
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "smoke_launch" => Some(smoke_launch()),
        "fov_smoke" => Some(fov_smoke()),
        "fov_grid_memory" => Some(fov_grid_memory()),
        "fov_cone_occlusion" => Some(fov_cone_occlusion()),
        _ => None,
    }
}

fn pause_motion(paused: bool) -> Action {
    Action::Custom(Box::new(move |world| set_pause_motion(world, paused)))
}

fn move_grid_viewer(cell: IVec2) -> Action {
    Action::Custom(Box::new(move |world| set_grid_viewer_cell(world, cell)))
}

fn guard_angle(angle: f32) -> Action {
    Action::Custom(Box::new(move |world| set_guard_angle(world, angle)))
}

fn build_smoke(name: &'static str) -> Scenario {
    Scenario::builder(name)
        .description("Boot the shared-crate lab, verify both the grid and cone viewers initialize, and capture the default mixed scene.")
        .then(pause_motion(true))
        .then(guard_angle(0.0))
        .then(Action::WaitFrames(8))
        .then(assertions::custom("grid visibility initialized", |world| {
            world.resource::<crate::LabDiagnostics>().grid_visible_cells > 0
        }))
        .then(assertions::custom("guard sees at least one target", |world| {
            world.resource::<crate::LabDiagnostics>().guard_visible_targets > 0
        }))
        .then(Action::Screenshot("smoke".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary(name))
        .build()
}

fn smoke_launch() -> Scenario {
    build_smoke("smoke_launch")
}

fn fov_smoke() -> Scenario {
    build_smoke("fov_smoke")
}

fn fov_grid_memory() -> Scenario {
    Scenario::builder("fov_grid_memory")
        .description("Move the grid viewer away from a known sample cell and verify it downgrades from visible to explored instead of vanishing.")
        .then(pause_motion(true))
        .then(move_grid_viewer(IVec2::new(2, 8)))
        .then(Action::WaitFrames(6))
        .then(assertions::custom("memory sample starts visible", |world| {
            world.resource::<crate::LabDiagnostics>().memory_sample_visible
        }))
        .then(Action::Screenshot("memory_visible".into()))
        .then(Action::WaitFrames(1))
        .then(move_grid_viewer(IVec2::new(12, 2)))
        .then(Action::WaitFrames(8))
        .then(assertions::custom("memory sample is no longer visible", |world| {
            !world.resource::<crate::LabDiagnostics>().memory_sample_visible
        }))
        .then(assertions::custom("memory sample stays explored", |world| {
            world.resource::<crate::LabDiagnostics>().memory_sample_explored
        }))
        .then(Action::Screenshot("memory_explored".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("fov_grid_memory"))
        .build()
}

fn fov_cone_occlusion() -> Scenario {
    Scenario::builder("fov_cone_occlusion")
        .description("Aim the guard straight across the arena and verify the front target is visible while the hidden target stays blocked behind the occluder.")
        .then(pause_motion(true))
        .then(guard_angle(0.0))
        .then(Action::WaitFrames(6))
        .then(assertions::custom("front target is visible", |world| {
            world.resource::<crate::LabDiagnostics>().front_target_visible
        }))
        .then(assertions::custom("hidden target stays occluded", |world| {
            !world.resource::<crate::LabDiagnostics>().hidden_target_visible
        }))
        .then(Action::Screenshot("cone_blocked".into()))
        .then(Action::WaitFrames(1))
        .then(guard_angle(-0.95))
        .then(Action::WaitFrames(6))
        .then(assertions::custom("front target drops out when the guard rotates away", |world| {
            !world.resource::<crate::LabDiagnostics>().front_target_visible
        }))
        .then(assertions::custom("hidden target remains remembered or hidden, never directly seen", |world| {
            !world.resource::<crate::LabDiagnostics>().hidden_target_visible
                && world.resource::<crate::LabDiagnostics>().remembered_targets <= 1
        }))
        .then(Action::Screenshot("cone_swept".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("fov_cone_occlusion"))
        .build()
}
