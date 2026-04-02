use bevy::prelude::*;

use crate::{GridMapSpec, grid::GridCornerPolicy};

pub fn supercover_line(start: IVec2, end: IVec2) -> Vec<IVec2> {
    let mut points = vec![start];
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let nx = dx.abs();
    let ny = dy.abs();
    let sign_x = dx.signum();
    let sign_y = dy.signum();

    let mut x = start.x;
    let mut y = start.y;
    let mut ix = 0;
    let mut iy = 0;

    while ix < nx || iy < ny {
        let decision = (1 + 2 * ix) * ny - (1 + 2 * iy) * nx;
        if decision == 0 {
            x += sign_x;
            y += sign_y;
            ix += 1;
            iy += 1;
        } else if decision < 0 {
            x += sign_x;
            ix += 1;
        } else {
            y += sign_y;
            iy += 1;
        }
        points.push(IVec2::new(x, y));
    }

    points
}

pub fn has_grid_line_of_sight(
    spec: GridMapSpec,
    origin: IVec2,
    target: IVec2,
    corner_policy: GridCornerPolicy,
    reveal_blockers: bool,
    mut is_opaque: impl FnMut(IVec2) -> bool,
) -> bool {
    if !spec.in_bounds(origin) || !spec.in_bounds(target) {
        return false;
    }
    if origin == target {
        return true;
    }

    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let nx = dx.abs();
    let ny = dy.abs();
    let sign_x = dx.signum();
    let sign_y = dy.signum();

    let mut current = origin;
    let mut ix = 0;
    let mut iy = 0;

    while ix < nx || iy < ny {
        let decision = (1 + 2 * ix) * ny - (1 + 2 * iy) * nx;
        if decision == 0 {
            let horizontal = current + IVec2::new(sign_x, 0);
            let vertical = current + IVec2::new(0, sign_y);

            let horizontal_opaque = !spec.in_bounds(horizontal) || is_opaque(horizontal);
            let vertical_opaque = !spec.in_bounds(vertical) || is_opaque(vertical);

            let blocked = match corner_policy {
                GridCornerPolicy::IgnoreAdjacentWalls => false,
                GridCornerPolicy::BlockIfBothAdjacentWalls => horizontal_opaque && vertical_opaque,
                GridCornerPolicy::BlockIfEitherAdjacentWall => horizontal_opaque || vertical_opaque,
            };

            if blocked {
                return false;
            }

            current += IVec2::new(sign_x, sign_y);
            ix += 1;
            iy += 1;
        } else if decision < 0 {
            current.x += sign_x;
            ix += 1;
        } else {
            current.y += sign_y;
            iy += 1;
        }

        if !spec.in_bounds(current) {
            return false;
        }

        let opaque = is_opaque(current);
        if current == target {
            return reveal_blockers || !opaque;
        }
        if opaque {
            return false;
        }
    }

    true
}

#[cfg(test)]
#[path = "los_tests.rs"]
mod tests;
