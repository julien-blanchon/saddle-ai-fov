use bevy::prelude::*;

use super::{has_grid_line_of_sight, supercover_line};
use crate::{GridMapSpec, grid::GridCornerPolicy};

fn spec() -> GridMapSpec {
    GridMapSpec {
        origin: Vec2::ZERO,
        dimensions: UVec2::new(10, 10),
        cell_size: Vec2::ONE,
    }
}

#[test]
fn supercover_line_tracks_corner_crossings() {
    let line = supercover_line(IVec2::new(1, 1), IVec2::new(4, 4));
    assert_eq!(line.first().copied(), Some(IVec2::new(1, 1)));
    assert_eq!(line.last().copied(), Some(IVec2::new(4, 4)));
}

#[test]
fn los_blocks_when_both_adjacent_walls_close_diagonal_gap() {
    let visible = has_grid_line_of_sight(
        spec(),
        IVec2::new(1, 1),
        IVec2::new(2, 2),
        GridCornerPolicy::BlockIfBothAdjacentWalls,
        true,
        |cell| matches!(cell, IVec2 { x: 2, y: 1 } | IVec2 { x: 1, y: 2 }),
    );
    assert!(!visible);
}

#[test]
fn los_allows_diagonal_gap_when_policy_ignores_adjacent_walls() {
    let visible = has_grid_line_of_sight(
        spec(),
        IVec2::new(1, 1),
        IVec2::new(2, 2),
        GridCornerPolicy::IgnoreAdjacentWalls,
        true,
        |cell| matches!(cell, IVec2 { x: 2, y: 1 } | IVec2 { x: 1, y: 2 }),
    );
    assert!(visible);
}

#[test]
fn los_blocks_when_either_adjacent_wall_policy_hits_single_wall() {
    let visible = has_grid_line_of_sight(
        spec(),
        IVec2::new(1, 1),
        IVec2::new(2, 2),
        GridCornerPolicy::BlockIfEitherAdjacentWall,
        true,
        |cell| cell == IVec2::new(2, 1),
    );
    assert!(!visible);
}

#[test]
fn blocker_target_is_hidden_when_reveal_blockers_is_disabled() {
    let visible = has_grid_line_of_sight(
        spec(),
        IVec2::new(1, 1),
        IVec2::new(3, 1),
        GridCornerPolicy::IgnoreAdjacentWalls,
        false,
        |cell| cell == IVec2::new(3, 1),
    );
    assert!(!visible);
}
