use bevy::prelude::*;

use crate::awareness::AwarenessLevel;

#[derive(Message, Clone, Debug)]
pub struct GridVisibilityChanged {
    pub viewer: Entity,
    pub entered: Vec<IVec2>,
    pub exited: Vec<IVec2>,
}

#[derive(Message, Clone, Debug)]
pub struct SpatialAwarenessChanged {
    pub viewer: Entity,
    pub target: Entity,
    pub previous_level: AwarenessLevel,
    pub level: AwarenessLevel,
    pub awareness: f32,
    pub last_known_position: Option<Vec3>,
}

#[derive(Message, Clone, Debug)]
pub struct SpatialTargetDetected {
    pub viewer: Entity,
    pub target: Entity,
    pub awareness: f32,
    pub last_known_position: Option<Vec3>,
}

#[derive(Message, Clone, Debug)]
pub struct SpatialTargetLost {
    pub viewer: Entity,
    pub target: Entity,
    pub awareness: f32,
    pub last_known_position: Option<Vec3>,
}
