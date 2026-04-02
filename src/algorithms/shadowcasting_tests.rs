use bevy::prelude::*;

use super::compute_grid_fov;
use crate::grid::{GridCornerPolicy, GridFovBackend, GridFovConfig, GridMapSpec};

fn spec(width: u32, height: u32) -> GridMapSpec {
    GridMapSpec {
        origin: Vec2::ZERO,
        dimensions: UVec2::new(width, height),
        cell_size: Vec2::ONE,
    }
}

fn sorted(mut cells: Vec<IVec2>) -> Vec<IVec2> {
    cells.sort_by_key(|cell| (cell.y, cell.x));
    cells
}

#[test]
fn empty_room_sees_all_cells_within_radius() {
    let result = compute_grid_fov(
        spec(5, 5),
        IVec2::new(2, 2),
        &GridFovConfig {
            radius: 2,
            ..default()
        },
        |_| false,
    );

    assert_eq!(
        sorted(result.visible_cells),
        sorted(vec![
            IVec2::new(2, 0),
            IVec2::new(1, 1),
            IVec2::new(2, 1),
            IVec2::new(3, 1),
            IVec2::new(0, 2),
            IVec2::new(1, 2),
            IVec2::new(2, 2),
            IVec2::new(3, 2),
            IVec2::new(4, 2),
            IVec2::new(1, 3),
            IVec2::new(2, 3),
            IVec2::new(3, 3),
            IVec2::new(2, 4),
        ])
    );
}

#[test]
fn doorway_blocks_cells_behind_the_wall() {
    let blockers = [
        IVec2::new(3, 1),
        IVec2::new(3, 2),
        IVec2::new(3, 4),
        IVec2::new(3, 5),
    ];
    let result = compute_grid_fov(
        spec(7, 7),
        IVec2::new(1, 3),
        &GridFovConfig {
            radius: 6,
            ..default()
        },
        |cell| blockers.contains(&cell),
    );

    assert!(result.visible_cells.contains(&IVec2::new(3, 3)));
    assert!(!result.visible_cells.contains(&IVec2::new(5, 1)));
    assert!(result.visible_cells.contains(&IVec2::new(5, 3)));
}

#[test]
fn pillars_create_shadowed_cells() {
    let result = compute_grid_fov(
        spec(9, 9),
        IVec2::new(2, 4),
        &GridFovConfig {
            radius: 6,
            ..default()
        },
        |cell| cell == IVec2::new(4, 4),
    );

    assert!(result.visible_cells.contains(&IVec2::new(4, 4)));
    assert!(!result.visible_cells.contains(&IVec2::new(6, 4)));
}

#[test]
fn corridor_keeps_far_cells_visible_but_not_side_walls() {
    let corridor_cells = [
        IVec2::new(1, 2),
        IVec2::new(2, 2),
        IVec2::new(3, 2),
        IVec2::new(4, 2),
        IVec2::new(5, 2),
    ];
    let result = compute_grid_fov(
        spec(7, 5),
        IVec2::new(1, 2),
        &GridFovConfig {
            radius: 6,
            ..default()
        },
        |cell| !corridor_cells.contains(&cell),
    );

    assert!(result.visible_cells.contains(&IVec2::new(5, 2)));
    assert!(!result.visible_cells.contains(&IVec2::new(5, 1)));
}

#[test]
fn t_junction_reveals_branch_cells_through_the_opening() {
    let open_cells = [
        IVec2::new(3, 1),
        IVec2::new(3, 2),
        IVec2::new(4, 2),
        IVec2::new(5, 2),
        IVec2::new(3, 3),
        IVec2::new(3, 4),
        IVec2::new(3, 5),
    ];
    let result = compute_grid_fov(
        spec(7, 7),
        IVec2::new(3, 2),
        &GridFovConfig {
            radius: 6,
            ..default()
        },
        |cell| !open_cells.contains(&cell),
    );

    assert!(result.visible_cells.contains(&IVec2::new(3, 1)));
    assert!(result.visible_cells.contains(&IVec2::new(4, 2)));
    assert!(result.visible_cells.contains(&IVec2::new(5, 2)));
}

#[test]
fn map_edges_clip_cleanly() {
    let result = compute_grid_fov(
        spec(4, 4),
        IVec2::new(0, 0),
        &GridFovConfig {
            radius: 3,
            ..default()
        },
        |_| false,
    );

    assert!(result.visible_cells.contains(&IVec2::new(0, 0)));
    assert!(result.visible_cells.contains(&IVec2::new(1, 1)));
    assert!(!result.visible_cells.contains(&IVec2::new(-1, 0)));
}

#[test]
fn reveal_blockers_false_hides_adjacent_walls() {
    let result = compute_grid_fov(
        spec(5, 5),
        IVec2::new(2, 2),
        &GridFovConfig {
            radius: 2,
            reveal_blockers: false,
            ..default()
        },
        |cell| cell == IVec2::new(3, 2),
    );

    assert!(!result.visible_cells.contains(&IVec2::new(3, 2)));
}

#[test]
fn zero_radius_only_returns_origin() {
    let result = compute_grid_fov(
        spec(6, 6),
        IVec2::new(2, 2),
        &GridFovConfig {
            radius: 0,
            ..default()
        },
        |_| false,
    );

    assert_eq!(result.visible_cells, vec![IVec2::new(2, 2)]);
}

#[test]
fn raycast_backend_matches_simple_corridor_case() {
    let blockers = [IVec2::new(4, 2)];
    let result = compute_grid_fov(
        spec(8, 5),
        IVec2::new(1, 2),
        &GridFovConfig {
            radius: 6,
            backend: GridFovBackend::RaycastLos,
            corner_policy: GridCornerPolicy::BlockIfBothAdjacentWalls,
            reveal_blockers: true,
        },
        |cell| blockers.contains(&cell),
    );

    assert!(result.visible_cells.contains(&IVec2::new(4, 2)));
    assert!(!result.visible_cells.contains(&IVec2::new(6, 2)));
}
