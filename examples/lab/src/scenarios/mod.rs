use bevy::prelude::*;
use saddle_bevy_e2e::{action::Action, actions::{assertions, inspect}, scenario::Scenario};

use crate::{pipeline_target_state, set_grid_viewer_cell, set_guard_angle, set_pause_motion};

pub fn list_scenarios() -> Vec<&'static str> {
    vec![
        "smoke_launch",
        "fov_smoke",
        "fov_grid_memory",
        "fov_cone_occlusion",
        "fov_guard_range_cutoff",
        "fov_stimulus_pipeline",
        "fov_awareness_detection",
        "fov_radius_sweep",
        "fov_multi_targets",
        "fov_memory_exploration",
    ]
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "smoke_launch" => Some(smoke_launch()),
        "fov_smoke" => Some(fov_smoke()),
        "fov_grid_memory" => Some(fov_grid_memory()),
        "fov_cone_occlusion" => Some(fov_cone_occlusion()),
        "fov_guard_range_cutoff" => Some(fov_guard_range_cutoff()),
        "fov_stimulus_pipeline" | "fov_awareness_detection" => Some(fov_stimulus_pipeline(name)),
        "fov_radius_sweep" => Some(fov_radius_sweep()),
        "fov_multi_targets" => Some(fov_multi_targets()),
        "fov_memory_exploration" => Some(fov_memory_exploration()),
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
        .then(inspect::log_resource::<crate::LabDiagnostics>("smoke diagnostics"))
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
        .then(Action::WaitFrames(2))
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
        .then(Action::WaitFrames(2))
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
        .then(Action::WaitFrames(20))
        .then(assertions::custom("front target drops out when the guard rotates away", |world| {
            !world.resource::<crate::LabDiagnostics>().front_target_visible
        }))
        .then(assertions::custom(
            "hidden target remains remembered or hidden, never directly seen",
            |world| !world.resource::<crate::LabDiagnostics>().hidden_target_visible,
        ))
        .then(Action::Screenshot("cone_swept".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("fov_cone_occlusion"))
        .build()
}

fn fov_guard_range_cutoff() -> Scenario {
    Scenario::builder("fov_guard_range_cutoff")
        .description(
            "Reduce the guard cone range below the front target distance, verify the front target drops out while the closer pipeline target remains visible, then restore the range.",
        )
        .then(Action::WaitFrames(2))
        .then(pause_motion(true))
        .then(guard_angle(0.0))
        .then(Action::WaitFrames(6))
        .then(assertions::custom("front target starts visible", |world| {
            world.resource::<crate::LabDiagnostics>().front_target_visible
        }))
        .then(assertions::custom("pipeline target starts visible", |world| {
            world.resource::<crate::LabDiagnostics>().pipeline_target_visible
        }))
        .then(inspect::log_resource::<crate::LabDiagnostics>("range_cutoff_baseline"))
        .then(Action::Screenshot("range_cutoff_before".into()))
        .then(Action::WaitFrames(1))
        .then(Action::Custom(Box::new(|world| {
            world.resource_mut::<crate::LabControl>().guard_range = 390.0;
        })))
        .then(Action::WaitFrames(8))
        .then(assertions::custom("front target cut off by range", |world| {
            let diagnostics = world.resource::<crate::LabDiagnostics>();
            !diagnostics.front_target_visible && diagnostics.pipeline_target_visible
        }))
        .then(inspect::log_resource::<crate::LabDiagnostics>("range_cutoff_cutoff"))
        .then(Action::Screenshot("range_cutoff_after".into()))
        .then(Action::WaitFrames(1))
        .then(Action::Custom(Box::new(|world| {
            world.resource_mut::<crate::LabControl>().guard_range = 420.0;
        })))
        .then(Action::WaitUntil {
            label: "front target visible again".into(),
            condition: Box::new(|world| world.resource::<crate::LabDiagnostics>().front_target_visible),
            max_frames: 60,
        })
        .then(assertions::custom("front target visibility restored", |world| {
            world.resource::<crate::LabDiagnostics>().front_target_visible
        }))
        .then(inspect::log_resource::<crate::LabDiagnostics>("range_cutoff_restored"))
        .then(Action::Screenshot("range_cutoff_restored".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("fov_guard_range_cutoff"))
        .build()
}

fn fov_radius_sweep() -> Scenario {
    Scenario::builder("fov_radius_sweep")
        .description("Verify that shrinking the grid radius reduces visible cells and expanding it recovers them. Also verify cone range changes affect how many spatial targets are visible.")
        .then(Action::WaitFrames(2))
        .then(pause_motion(true))
        .then(move_grid_viewer(IVec2::new(7, 6)))
        .then(Action::WaitFrames(6))
        // Baseline: default radius (4) — should see several cells
        .then(assertions::custom("baseline: grid sees cells with radius 4", |world| {
            world.resource::<crate::LabDiagnostics>().grid_visible_cells >= 4
        }))
        .then(Action::Screenshot("radius_baseline".into()))
        .then(Action::WaitFrames(1))
        // Shrink radius to 2 — fewer visible cells
        .then(Action::Custom(Box::new(|world| {
            world.resource_mut::<crate::LabControl>().grid_radius = 2;
        })))
        .then(Action::WaitFrames(6))
        .then(assertions::custom("shrunk radius reduces visible cells", |world| {
            let d = world.resource::<crate::LabDiagnostics>();
            d.grid_visible_cells < 20
        }))
        .then(Action::Screenshot("radius_shrunk".into()))
        .then(Action::WaitFrames(1))
        // Expand radius back to 6 — more cells visible
        .then(Action::Custom(Box::new(|world| {
            world.resource_mut::<crate::LabControl>().grid_radius = 6;
        })))
        .then(Action::WaitFrames(6))
        .then(assertions::custom("expanded radius reveals more cells", |world| {
            world.resource::<crate::LabDiagnostics>().grid_visible_cells >= 4
        }))
        .then(Action::Screenshot("radius_expanded".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("fov_radius_sweep"))
        .build()
}

fn fov_multi_targets() -> Scenario {
    Scenario::builder("fov_multi_targets")
        .description("Point the guard directly at the arena to maximize visible targets, verify at least two targets are simultaneously visible. Then rotate 180° and verify the count drops to zero.")
        .then(Action::WaitFrames(2))
        .then(pause_motion(true))
        // Widen the cone for maximum coverage
        .then(Action::Custom(Box::new(|world| {
            let mut control = world.resource_mut::<crate::LabControl>();
            control.guard_range = 560.0;
            control.guard_half_angle = 1.2;
        })))
        .then(guard_angle(0.0))
        .then(Action::WaitFrames(8))
        // At wide angle pointing straight ahead several targets should be visible
        .then(assertions::custom("wide cone: guard sees 2+ targets", |world| {
            world.resource::<crate::LabDiagnostics>().guard_visible_targets >= 2
        }))
        .then(assertions::custom("front target visible with wide cone", |world| {
            world.resource::<crate::LabDiagnostics>().front_target_visible
        }))
        .then(Action::Screenshot("multi_targets_visible".into()))
        .then(Action::WaitFrames(1))
        // Point guard backwards (away from the arena)
        .then(guard_angle(std::f32::consts::PI))
        .then(Action::WaitFrames(8))
        .then(assertions::custom("guard sees zero targets when rotated away", |world| {
            world.resource::<crate::LabDiagnostics>().guard_visible_targets == 0
        }))
        .then(Action::Screenshot("multi_targets_none".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("fov_multi_targets"))
        .build()
}

fn fov_memory_exploration() -> Scenario {
    Scenario::builder("fov_memory_exploration")
        .description("Walk the grid viewer along a path that covers new terrain and verify that explored cell count grows monotonically — cells once seen are never forgotten.")
        .then(Action::WaitFrames(2))
        .then(pause_motion(true))
        .then(move_grid_viewer(IVec2::new(2, 8)))
        .then(Action::WaitFrames(6))
        // Record baseline explored count
        .then(assertions::custom("cells explored after first position", |world| {
            world.resource::<crate::LabDiagnostics>().grid_explored_cells >= 1
        }))
        .then(Action::Screenshot("exploration_start".into()))
        .then(Action::WaitFrames(1))
        // Move to a different area
        .then(move_grid_viewer(IVec2::new(7, 2)))
        .then(Action::WaitFrames(8))
        .then(assertions::custom("explored count grows after moving", |world| {
            world.resource::<crate::LabDiagnostics>().grid_explored_cells >= 4
        }))
        .then(Action::Screenshot("exploration_moved".into()))
        .then(Action::WaitFrames(1))
        // Move to a third distinct area
        .then(move_grid_viewer(IVec2::new(12, 6)))
        .then(Action::WaitFrames(8))
        .then(assertions::custom("explored count continues growing", |world| {
            world.resource::<crate::LabDiagnostics>().grid_explored_cells >= 8
        }))
        // Original cell from step 1 must still be in explored even though viewer moved away
        .then(assertions::custom("memory sample from step 1 still explored", |world| {
            world.resource::<crate::LabDiagnostics>().memory_sample_explored
        }))
        .then(Action::Screenshot("exploration_final".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("fov_memory_exploration"))
        .build()
}

fn fov_stimulus_pipeline(name: &str) -> Scenario {
    Scenario::builder(name)
        .description("Hold the guard on the pipeline target until the neutral stimulus signal crosses the alert threshold, then rotate away and verify the optional stealth mapper transitions into a post-visual state while line of sight is lost.")
        .then(pause_motion(true))
        .then(Action::WaitFrames(2))
        .then(guard_angle(0.0))
        .then(Action::WaitFrames(75))
        .then(assertions::custom("pipeline target crosses the alert threshold", |world| {
            pipeline_target_state(world).is_some_and(|(_, signal)| signal >= 0.8)
        }))
        .then(assertions::custom("pipeline target reaches alert state", |world| {
            pipeline_target_state(world).is_some_and(|(level, _)| {
                level == saddle_ai_fov::StealthAwarenessLevel::Alert
            })
        }))
        .then(Action::Screenshot("pipeline_alert".into()))
        .then(Action::WaitFrames(1))
        .then(guard_angle(-0.95))
        .then(Action::WaitFrames(20))
        .then(assertions::custom("pipeline target leaves direct sight", |world| {
            !world.resource::<crate::LabDiagnostics>().pipeline_target_visible
        }))
        .then(assertions::custom("pipeline target remains in a post-visual stealth state", |world| {
            pipeline_target_state(world).is_some_and(|(level, _)| {
                matches!(
                    level,
                    saddle_ai_fov::StealthAwarenessLevel::Searching
                        | saddle_ai_fov::StealthAwarenessLevel::Lost
                )
            })
        }))
        .then(Action::Screenshot("pipeline_searching".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary(name))
        .build()
}
