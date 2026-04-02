use bevy::prelude::*;

use crate::{
    grid::GridFovConfig,
    spatial::{OccluderShape, SpatialDimension, SpatialShape, VisibilityLayerMask},
};

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct FovDirty;

#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct GridFov {
    pub config: GridFovConfig,
    pub remember_seen_cells: bool,
    pub enabled: bool,
}

impl GridFov {
    pub fn new(radius: i32) -> Self {
        Self {
            config: GridFovConfig {
                radius,
                ..default()
            },
            remember_seen_cells: true,
            enabled: true,
        }
    }

    pub fn with_config(mut self, config: GridFovConfig) -> Self {
        self.config = config;
        self
    }
}

impl Default for GridFov {
    fn default() -> Self {
        Self::new(GridFovConfig::default().radius)
    }
}

#[derive(Component, Debug, Clone, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct GridFovState {
    pub visible_now: Vec<IVec2>,
    pub explored: Vec<IVec2>,
    pub entered: Vec<IVec2>,
    pub exited: Vec<IVec2>,
}

impl GridFovState {
    pub fn contains(&self, cell: IVec2) -> bool {
        self.visible_now.contains(&cell)
    }
}

#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct SpatialFov {
    pub shape: SpatialShape,
    pub dimension: SpatialDimension,
    pub layers: VisibilityLayerMask,
    pub local_origin: Vec3,
    pub local_forward: Vec3,
    pub near_override: f32,
    pub remember_seen_targets: bool,
    pub enabled: bool,
}

impl SpatialFov {
    pub fn radius(range: f32) -> Self {
        Self {
            shape: SpatialShape::Radius {
                range: range.max(0.0),
            },
            dimension: SpatialDimension::Planar2d,
            layers: VisibilityLayerMask::ALL,
            local_origin: Vec3::ZERO,
            local_forward: Vec3::X,
            near_override: 0.0,
            remember_seen_targets: true,
            enabled: true,
        }
    }

    pub fn cone_2d(range: f32, half_angle_radians: f32) -> Self {
        Self {
            shape: SpatialShape::Cone {
                range: range.max(0.0),
                half_angle_radians: half_angle_radians.max(0.0),
            },
            dimension: SpatialDimension::Planar2d,
            layers: VisibilityLayerMask::ALL,
            local_origin: Vec3::ZERO,
            local_forward: Vec3::X,
            near_override: 0.0,
            remember_seen_targets: true,
            enabled: true,
        }
    }

    pub fn cone_3d(range: f32, half_angle_radians: f32) -> Self {
        Self {
            shape: SpatialShape::Cone {
                range: range.max(0.0),
                half_angle_radians: half_angle_radians.max(0.0),
            },
            dimension: SpatialDimension::Volumetric3d,
            layers: VisibilityLayerMask::ALL,
            local_origin: Vec3::ZERO,
            local_forward: Vec3::X,
            near_override: 0.0,
            remember_seen_targets: true,
            enabled: true,
        }
    }

    pub fn with_layers(mut self, layers: VisibilityLayerMask) -> Self {
        self.layers = layers;
        self
    }

    pub fn with_local_origin(mut self, local_origin: Vec3) -> Self {
        self.local_origin = local_origin;
        self
    }

    pub fn with_local_forward(mut self, local_forward: Vec3) -> Self {
        self.local_forward = local_forward;
        self
    }

    pub fn with_near_override(mut self, near_override: f32) -> Self {
        self.near_override = near_override.max(0.0);
        self
    }
}

#[derive(Component, Debug, Clone, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct SpatialFovState {
    pub visible_now: Vec<Entity>,
    pub remembered: Vec<Entity>,
    pub entered: Vec<Entity>,
    pub exited: Vec<Entity>,
}

impl SpatialFovState {
    pub fn contains(&self, entity: Entity) -> bool {
        self.visible_now.contains(&entity)
    }
}

#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct FovTarget {
    pub layers: VisibilityLayerMask,
    pub sample_points: Vec<Vec3>,
    pub enabled: bool,
}

impl FovTarget {
    pub fn new() -> Self {
        Self {
            layers: VisibilityLayerMask::ALL,
            sample_points: vec![Vec3::ZERO],
            enabled: true,
        }
    }

    pub fn with_layers(mut self, layers: VisibilityLayerMask) -> Self {
        self.layers = layers;
        self
    }

    pub fn with_sample_points(mut self, sample_points: Vec<Vec3>) -> Self {
        self.sample_points = if sample_points.is_empty() {
            vec![Vec3::ZERO]
        } else {
            sample_points
        };
        self
    }
}

impl Default for FovTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct FovOccluder {
    pub layers: VisibilityLayerMask,
    pub shape: OccluderShape,
    pub local_offset: Vec3,
    pub enabled: bool,
}

impl FovOccluder {
    pub fn new(shape: OccluderShape) -> Self {
        Self {
            layers: VisibilityLayerMask::ALL,
            shape,
            local_offset: Vec3::ZERO,
            enabled: true,
        }
    }

    pub fn with_layers(mut self, layers: VisibilityLayerMask) -> Self {
        self.layers = layers;
        self
    }

    pub fn with_local_offset(mut self, local_offset: Vec3) -> Self {
        self.local_offset = local_offset;
        self
    }
}
