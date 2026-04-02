use std::collections::HashSet;

use bevy::prelude::*;
use saddle_ai_fov::{GridMapSpec, GridOpacityMap};

pub const DEMO_GRID: &[&str] = &[
    "###############",
    "#.............#",
    "#.###.#####.#.#",
    "#.#...#...#.#.#",
    "#.#.###.#.#.#.#",
    "#...#...#...#.#",
    "###.#.#####.#.#",
    "#...#.....#...#",
    "#.#######.###.#",
    "#.............#",
    "###############",
];

#[derive(Component)]
pub struct GridCellSprite(pub IVec2);

#[allow(dead_code)]
pub fn demo_grid_map(cell_size: f32) -> GridOpacityMap {
    let height = DEMO_GRID.len() as u32;
    let width = DEMO_GRID[0].len() as u32;
    let spec = GridMapSpec {
        origin: Vec2::new(
            -(width as f32 * cell_size) * 0.5,
            -(height as f32 * cell_size) * 0.5,
        ),
        dimensions: UVec2::new(width, height),
        cell_size: Vec2::splat(cell_size),
    };

    GridOpacityMap::from_fn(spec, |cell| {
        DEMO_GRID[cell.y as usize].as_bytes()[cell.x as usize] == b'#'
    })
}

pub fn spawn_grid_tiles(commands: &mut Commands, grid: &GridOpacityMap) {
    for y in 0..grid.spec.dimensions.y as i32 {
        for x in 0..grid.spec.dimensions.x as i32 {
            let cell = IVec2::new(x, y);
            let center = grid
                .spec
                .cell_to_world_center(cell)
                .expect("grid cell should map to a world center");
            let color = if grid.is_opaque(cell) {
                Color::srgb(0.18, 0.19, 0.22)
            } else {
                Color::srgb(0.08, 0.09, 0.11)
            };

            commands.spawn((
                Name::new(format!("Grid Cell {x},{y}")),
                GridCellSprite(cell),
                Sprite {
                    color,
                    custom_size: Some(grid.spec.cell_size - Vec2::splat(1.5)),
                    ..default()
                },
                Transform::from_translation(center.extend(0.0)),
            ));
        }
    }
}

pub fn grid_world_position(spec: &GridMapSpec, cell: IVec2, z: f32) -> Vec3 {
    spec.cell_to_world_center(cell)
        .expect("path cells must be in bounds")
        .extend(z)
}

pub fn sample_path(
    spec: &GridMapSpec,
    path: &[IVec2],
    elapsed_secs: f32,
    speed: f32,
    z: f32,
) -> Vec3 {
    let progress = elapsed_secs * speed;
    let from = progress.floor() as usize % path.len();
    let to = (from + 1) % path.len();
    let t = progress.fract();
    let start = grid_world_position(spec, path[from], z);
    let end = grid_world_position(spec, path[to], z);
    start.lerp(end, t)
}

pub fn apply_grid_visibility_colors(
    grid: &GridOpacityMap,
    visible: &[IVec2],
    explored: &[IVec2],
    tiles: &mut Query<(&GridCellSprite, &mut Sprite)>,
) {
    let visible: HashSet<_> = visible.iter().copied().collect();
    let explored: HashSet<_> = explored.iter().copied().collect();

    for (cell, mut sprite) in tiles.iter_mut() {
        sprite.color = if visible.contains(&cell.0) {
            if grid.is_opaque(cell.0) {
                Color::srgb(0.80, 0.74, 0.52)
            } else {
                Color::srgb(0.19, 0.65, 0.88)
            }
        } else if explored.contains(&cell.0) {
            if grid.is_opaque(cell.0) {
                Color::srgb(0.30, 0.29, 0.25)
            } else {
                Color::srgb(0.13, 0.17, 0.20)
            }
        } else if grid.is_opaque(cell.0) {
            Color::srgb(0.18, 0.19, 0.22)
        } else {
            Color::srgb(0.08, 0.09, 0.11)
        };
    }
}
