use std::collections::HashSet;

use bevy::prelude::*;

use crate::components::GridFovState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum GridFovBackend {
    RecursiveShadowcasting,
    RaycastLos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum GridCornerPolicy {
    IgnoreAdjacentWalls,
    BlockIfBothAdjacentWalls,
    BlockIfEitherAdjacentWall,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct GridFovConfig {
    pub radius: i32,
    pub backend: GridFovBackend,
    pub corner_policy: GridCornerPolicy,
    pub reveal_blockers: bool,
}

impl Default for GridFovConfig {
    fn default() -> Self {
        Self {
            radius: 8,
            backend: GridFovBackend::RecursiveShadowcasting,
            corner_policy: GridCornerPolicy::BlockIfBothAdjacentWalls,
            reveal_blockers: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct GridFovResult {
    pub visible_cells: Vec<IVec2>,
    pub cells_considered: usize,
}

impl GridFovResult {
    pub fn empty() -> Self {
        Self {
            visible_cells: Vec::new(),
            cells_considered: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct GridMapSpec {
    pub origin: Vec2,
    pub dimensions: UVec2,
    pub cell_size: Vec2,
}

impl Default for GridMapSpec {
    fn default() -> Self {
        Self {
            origin: Vec2::ZERO,
            dimensions: UVec2::new(32, 24),
            cell_size: Vec2::splat(1.0),
        }
    }
}

impl GridMapSpec {
    pub fn world_size(&self) -> Vec2 {
        self.dimensions.as_vec2() * self.cell_size
    }

    pub fn cell_count(&self) -> usize {
        (self.dimensions.x * self.dimensions.y) as usize
    }

    pub fn in_bounds(&self, cell: IVec2) -> bool {
        cell.x >= 0
            && cell.y >= 0
            && (cell.x as u32) < self.dimensions.x
            && (cell.y as u32) < self.dimensions.y
    }

    pub fn index(&self, cell: IVec2) -> Option<usize> {
        self.in_bounds(cell)
            .then_some((cell.y as usize * self.dimensions.x as usize) + cell.x as usize)
    }

    pub fn cell_from_index(&self, index: usize) -> IVec2 {
        let width = self.dimensions.x as usize;
        IVec2::new((index % width) as i32, (index / width) as i32)
    }

    pub fn world_to_cell(&self, world: Vec2) -> Option<IVec2> {
        let local = world - self.origin;
        if local.x < 0.0 || local.y < 0.0 {
            return None;
        }

        let x = (local.x / self.cell_size.x).floor() as i32;
        let y = (local.y / self.cell_size.y).floor() as i32;
        let cell = IVec2::new(x, y);
        self.in_bounds(cell).then_some(cell)
    }

    pub fn cell_to_world_center(&self, cell: IVec2) -> Option<Vec2> {
        self.in_bounds(cell)
            .then_some(self.origin + (cell.as_vec2() * self.cell_size) + (self.cell_size * 0.5))
    }

    pub fn cell_to_world_rect(&self, cell: IVec2) -> Option<(Vec2, Vec2)> {
        self.in_bounds(cell).then_some({
            let min = self.origin + cell.as_vec2() * self.cell_size;
            let max = min + self.cell_size;
            (min, max)
        })
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct GridOpacityMap {
    pub spec: GridMapSpec,
    opaque_cells: Vec<bool>,
}

impl Default for GridOpacityMap {
    fn default() -> Self {
        Self::new(GridMapSpec::default())
    }
}

impl GridOpacityMap {
    pub fn new(spec: GridMapSpec) -> Self {
        Self {
            spec,
            opaque_cells: vec![false; spec.cell_count()],
        }
    }

    pub fn from_fn(spec: GridMapSpec, mut is_opaque: impl FnMut(IVec2) -> bool) -> Self {
        let mut map = Self::new(spec);
        for y in 0..spec.dimensions.y as i32 {
            for x in 0..spec.dimensions.x as i32 {
                let cell = IVec2::new(x, y);
                map.set_opaque(cell, is_opaque(cell));
            }
        }
        map
    }

    pub fn clear(&mut self) {
        self.opaque_cells.fill(false);
    }

    pub fn set_opaque(&mut self, cell: IVec2, opaque: bool) {
        let Some(index) = self.spec.index(cell) else {
            return;
        };
        self.opaque_cells[index] = opaque;
    }

    pub fn is_opaque(&self, cell: IVec2) -> bool {
        self.spec
            .index(cell)
            .and_then(|index| self.opaque_cells.get(index))
            .copied()
            .unwrap_or(true)
    }
}

pub fn merge_grid_visibility<'a>(states: impl IntoIterator<Item = &'a GridFovState>) -> Vec<IVec2> {
    let mut merged = HashSet::new();
    for state in states {
        merged.extend(state.visible_now.iter().copied());
    }

    let mut merged_vec: Vec<_> = merged.into_iter().collect();
    merged_vec.sort_by_key(|cell| (cell.y, cell.x));
    merged_vec
}
