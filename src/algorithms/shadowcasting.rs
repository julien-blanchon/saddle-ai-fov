use std::collections::HashSet;

use bevy::prelude::*;

use crate::{
    algorithms::los::has_grid_line_of_sight,
    grid::{GridFovBackend, GridFovConfig, GridFovResult, GridMapSpec},
};

pub fn compute_grid_fov(
    spec: GridMapSpec,
    origin: IVec2,
    config: &GridFovConfig,
    mut is_opaque: impl FnMut(IVec2) -> bool,
) -> GridFovResult {
    if !spec.in_bounds(origin) {
        return GridFovResult::empty();
    }

    match config.backend {
        GridFovBackend::RecursiveShadowcasting => {
            recursive_shadowcasting(spec, origin, config, &mut is_opaque)
        }
        GridFovBackend::RaycastLos => raycast_visibility(spec, origin, config, &mut is_opaque),
    }
}

fn recursive_shadowcasting(
    spec: GridMapSpec,
    origin: IVec2,
    config: &GridFovConfig,
    is_opaque: &mut impl FnMut(IVec2) -> bool,
) -> GridFovResult {
    let mut visible = HashSet::new();
    let mut cells_considered = 1;
    visible.insert(origin);

    for octant in 0..8 {
        cast_light(
            spec,
            origin,
            config.radius.max(0),
            1,
            1.0,
            0.0,
            octant,
            &mut visible,
            &mut cells_considered,
            is_opaque,
        );
    }

    finalize_visibility(spec, origin, config, visible, is_opaque, cells_considered)
}

fn cast_light(
    spec: GridMapSpec,
    origin: IVec2,
    radius: i32,
    row: i32,
    mut start_slope: f32,
    end_slope: f32,
    octant: u8,
    visible: &mut HashSet<IVec2>,
    cells_considered: &mut usize,
    is_opaque: &mut impl FnMut(IVec2) -> bool,
) {
    if start_slope < end_slope {
        return;
    }

    let mut next_start_slope = start_slope;

    for distance in row..=radius {
        let mut blocked = false;
        let dy = -distance;

        for dx in -distance..=0 {
            let (mx, my) = transform_octant(dx, dy, octant);
            let cell = origin + IVec2::new(mx, my);

            let left_slope = (dx as f32 - 0.5) / (dy as f32 + 0.5);
            let right_slope = (dx as f32 + 0.5) / (dy as f32 - 0.5);

            if start_slope < right_slope {
                continue;
            }
            if end_slope > left_slope {
                break;
            }

            *cells_considered += 1;

            let in_radius = dx * dx + dy * dy <= radius * radius;
            let opaque = !spec.in_bounds(cell) || is_opaque(cell);

            if in_radius && spec.in_bounds(cell) {
                visible.insert(cell);
            }

            if blocked {
                if opaque {
                    next_start_slope = right_slope;
                } else {
                    blocked = false;
                    start_slope = next_start_slope;
                }
            } else if opaque && distance < radius {
                blocked = true;
                cast_light(
                    spec,
                    origin,
                    radius,
                    distance + 1,
                    start_slope,
                    left_slope,
                    octant,
                    visible,
                    cells_considered,
                    is_opaque,
                );
                next_start_slope = right_slope;
            }
        }

        if blocked {
            break;
        }
    }
}

fn raycast_visibility(
    spec: GridMapSpec,
    origin: IVec2,
    config: &GridFovConfig,
    is_opaque: &mut impl FnMut(IVec2) -> bool,
) -> GridFovResult {
    let radius = config.radius.max(0);
    let mut visible = HashSet::new();
    let mut cells_considered = 1;
    visible.insert(origin);

    let min_y = (origin.y - radius).max(0);
    let max_y = (origin.y + radius).min(spec.dimensions.y as i32 - 1);
    let min_x = (origin.x - radius).max(0);
    let max_x = (origin.x + radius).min(spec.dimensions.x as i32 - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let cell = IVec2::new(x, y);
            if cell == origin {
                continue;
            }

            let delta = cell - origin;
            if delta.x * delta.x + delta.y * delta.y > radius * radius {
                continue;
            }

            cells_considered += 1;
            if has_grid_line_of_sight(
                spec,
                origin,
                cell,
                config.corner_policy,
                config.reveal_blockers,
                &mut *is_opaque,
            ) {
                visible.insert(cell);
            }
        }
    }

    finalize_visibility(spec, origin, config, visible, is_opaque, cells_considered)
}

fn finalize_visibility(
    spec: GridMapSpec,
    origin: IVec2,
    config: &GridFovConfig,
    visible: HashSet<IVec2>,
    is_opaque: &mut impl FnMut(IVec2) -> bool,
    cells_considered: usize,
) -> GridFovResult {
    let mut final_cells = Vec::new();
    for cell in visible {
        if cell != origin
            && !has_grid_line_of_sight(
                spec,
                origin,
                cell,
                config.corner_policy,
                config.reveal_blockers,
                &mut *is_opaque,
            )
        {
            continue;
        }

        if cell != origin && !config.reveal_blockers && is_opaque(cell) {
            continue;
        }

        final_cells.push(cell);
    }

    final_cells.sort_by_key(|cell| (cell.y, cell.x));
    final_cells.dedup();

    GridFovResult {
        visible_cells: final_cells,
        cells_considered,
    }
}

fn transform_octant(col: i32, row: i32, octant: u8) -> (i32, i32) {
    match octant {
        0 => (col, row),
        1 => (row, col),
        2 => (row, -col),
        3 => (col, -row),
        4 => (-col, -row),
        5 => (-row, -col),
        6 => (-row, col),
        7 => (-col, row),
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[path = "shadowcasting_tests.rs"]
mod tests;
