use bevy::prelude::*;

#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct FovRuntimeConfig {
    pub max_viewers_per_frame: usize,
}

impl Default for FovRuntimeConfig {
    fn default() -> Self {
        Self {
            max_viewers_per_frame: usize::MAX,
        }
    }
}
