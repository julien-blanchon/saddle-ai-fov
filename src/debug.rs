use bevy::{gizmos::config::GizmoConfigGroup, prelude::*};

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct FovDebugGizmos;

#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct FovDebugSettings {
    pub enabled: bool,
    pub draw_grid_cells: bool,
    pub draw_view_shapes: bool,
    pub draw_filled_shapes: bool,
    pub draw_occlusion_rays: bool,
    pub draw_blocked_rays: bool,
    pub draw_occluder_shapes: bool,
    pub max_grid_cells_per_viewer: usize,
}

impl Default for FovDebugSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            draw_grid_cells: true,
            draw_view_shapes: true,
            draw_filled_shapes: true,
            draw_occlusion_rays: true,
            draw_blocked_rays: false,
            draw_occluder_shapes: true,
            max_grid_cells_per_viewer: 96,
        }
    }
}
