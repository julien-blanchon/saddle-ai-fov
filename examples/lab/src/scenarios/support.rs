use bevy::prelude::*;
use saddle_bevy_e2e::action::Action;

pub(super) fn freeze_and_aim_guard(angle: f32, settle_frames: u32) -> Vec<Action> {
    let mut actions = vec![
        super::pause_motion(true),
        super::guard_angle(angle),
    ];

    if settle_frames > 0 {
        actions.push(Action::WaitFrames(settle_frames));
    }

    actions
}

pub(super) fn move_grid_viewer_and_settle(cell: IVec2, settle_frames: u32) -> Vec<Action> {
    let mut actions = vec![super::move_grid_viewer(cell)];

    if settle_frames > 0 {
        actions.push(Action::WaitFrames(settle_frames));
    }

    actions
}
