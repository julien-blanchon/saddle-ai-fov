use bevy::prelude::*;

#[derive(Default, Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct FovStats {
    pub dirty_viewers: usize,
    pub recomputed_viewers: usize,
    pub visible_cells_total: usize,
    pub visible_targets_total: usize,
    pub target_checks: usize,
    pub occlusion_tests: usize,
    pub last_recompute_micros: u64,
}
