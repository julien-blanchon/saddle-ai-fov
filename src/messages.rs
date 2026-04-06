use bevy::prelude::*;

use crate::stealth::StealthAwarenessLevel;

#[derive(Message, Clone, Debug)]
pub struct GridVisibilityChanged {
    pub viewer: Entity,
    pub entered: Vec<IVec2>,
    pub exited: Vec<IVec2>,
}

#[derive(Message, Clone, Debug)]
pub struct SpatialVisibilityChanged {
    pub viewer: Entity,
    pub entered: Vec<Entity>,
    pub exited: Vec<Entity>,
}

#[derive(Message, Clone, Debug)]
pub struct SpatialStimulusChanged {
    pub viewer: Entity,
    pub target: Entity,
    pub previous_signal: f32,
    pub signal: f32,
    pub visibility_score: f32,
    pub indirect_signal: f32,
    pub currently_visible: bool,
    pub in_range: bool,
    pub inside_shape: bool,
    pub occluded: bool,
    pub last_known_position: Option<Vec3>,
}

#[derive(Message, Clone, Debug)]
pub struct StealthAwarenessChanged {
    pub viewer: Entity,
    pub target: Entity,
    pub previous_level: StealthAwarenessLevel,
    pub level: StealthAwarenessLevel,
    pub signal: f32,
    pub last_known_position: Option<Vec3>,
}

#[derive(Message, Clone, Debug)]
pub struct StealthTargetDetected {
    pub viewer: Entity,
    pub target: Entity,
    pub signal: f32,
    pub last_known_position: Option<Vec3>,
}

#[derive(Message, Clone, Debug)]
pub struct StealthTargetLost {
    pub viewer: Entity,
    pub target: Entity,
    pub signal: f32,
    pub last_known_position: Option<Vec3>,
}
