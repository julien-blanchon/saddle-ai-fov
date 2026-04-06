use bevy::prelude::*;

use crate::spatial::{SpatialDimension, SpatialShape, SpatialVisibilityQuery};

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct SpatialStimulusConfig {
    pub enabled: bool,
    pub max_signal: f32,
    pub gain_per_second: f32,
    pub loss_per_second: f32,
    pub focused_half_angle_radians: f32,
    pub focused_gain_multiplier: f32,
    pub peripheral_gain_multiplier: f32,
    pub distance_falloff_exponent: f32,
    pub minimum_visibility_factor: f32,
    pub indirect_gain_per_second: f32,
    pub forget_after_seconds: f32,
}

impl Default for SpatialStimulusConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_signal: 1.0,
            gain_per_second: 0.9,
            loss_per_second: 0.45,
            focused_half_angle_radians: 0.24,
            focused_gain_multiplier: 1.35,
            peripheral_gain_multiplier: 0.55,
            distance_falloff_exponent: 1.2,
            minimum_visibility_factor: 0.18,
            indirect_gain_per_second: 0.22,
            forget_after_seconds: 8.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SpatialStimulusEntry {
    pub entity: Entity,
    pub signal: f32,
    pub visibility_score: f32,
    pub indirect_signal: f32,
    pub currently_visible: bool,
    pub in_range: bool,
    pub inside_shape: bool,
    pub occluded: bool,
    pub focused: bool,
    pub last_seen_seconds_ago: Option<f32>,
    pub last_known_position: Option<Vec3>,
}

impl SpatialStimulusEntry {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            signal: 0.0,
            visibility_score: 0.0,
            indirect_signal: 0.0,
            currently_visible: false,
            in_range: false,
            inside_shape: false,
            occluded: false,
            focused: false,
            last_seen_seconds_ago: None,
            last_known_position: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SpatialStimulusScore {
    pub visibility_score: f32,
    pub distance_factor: f32,
    pub focused: bool,
}

pub(crate) fn should_forget_target(
    forget_after_seconds: f32,
    last_seen_seconds_ago: Option<f32>,
) -> bool {
    last_seen_seconds_ago.is_some_and(|elapsed| elapsed > forget_after_seconds.max(0.0))
}

pub(crate) fn visibility_score_for_sample(
    query: &SpatialVisibilityQuery,
    sample: Vec3,
    stimulus: &SpatialStimulusConfig,
) -> SpatialStimulusScore {
    let range = query.shape.range().max(0.001);
    let distance = sample_distance(query.dimension, query.origin, sample);
    let normalized_distance = 1.0 - (distance / range).clamp(0.0, 1.0);
    let distance_factor = stimulus.minimum_visibility_factor.clamp(0.0, 1.0)
        + (1.0 - stimulus.minimum_visibility_factor.clamp(0.0, 1.0))
            * normalized_distance.powf(stimulus.distance_falloff_exponent.max(0.01));

    let focused = is_focused(query, sample, stimulus.focused_half_angle_radians);
    let angular_factor = if focused {
        stimulus.focused_gain_multiplier.max(0.0)
    } else {
        stimulus.peripheral_gain_multiplier.max(0.0)
    };

    SpatialStimulusScore {
        visibility_score: distance_factor * angular_factor,
        distance_factor,
        focused,
    }
}

fn is_focused(
    query: &SpatialVisibilityQuery,
    sample: Vec3,
    focused_half_angle_radians: f32,
) -> bool {
    let focused_half_angle_radians = focused_half_angle_radians.max(0.0);
    match query.shape {
        SpatialShape::Radius { .. } => true,
        SpatialShape::Cone {
            half_angle_radians, ..
        } => {
            let focus_limit = focused_half_angle_radians.min(half_angle_radians.max(0.0));
            let Some(forward) = normalize_direction(query.dimension, query.forward) else {
                return true;
            };
            let Some(direction) = normalize_direction(query.dimension, sample - query.origin)
            else {
                return true;
            };
            direction.dot(forward) >= focus_limit.cos() - 0.0001
        }
        SpatialShape::Rect { .. } => {
            let Some(forward) = normalize_direction(query.dimension, query.forward) else {
                return true;
            };
            let Some(direction) = normalize_direction(query.dimension, sample - query.origin)
            else {
                return true;
            };
            direction.dot(forward) >= focused_half_angle_radians.cos() - 0.0001
        }
    }
}

fn normalize_direction(dimension: SpatialDimension, vector: Vec3) -> Option<Vec3> {
    let flattened = match dimension {
        SpatialDimension::Planar2d => Vec3::new(vector.x, vector.y, 0.0),
        SpatialDimension::Volumetric3d => vector,
    };

    (flattened.length_squared() > f32::EPSILON).then_some(flattened.normalize())
}

fn sample_distance(dimension: SpatialDimension, start: Vec3, end: Vec3) -> f32 {
    match dimension {
        SpatialDimension::Planar2d => start.truncate().distance(end.truncate()),
        SpatialDimension::Volumetric3d => start.distance(end),
    }
}

#[cfg(test)]
#[path = "stimulus_tests.rs"]
mod tests;
