use bevy::prelude::*;

use crate::spatial::{SpatialDimension, SpatialShape, SpatialVisibilityQuery};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum AwarenessLevel {
    Unaware,
    Suspicious,
    Alert,
    Searching,
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct SpatialAwarenessConfig {
    pub enabled: bool,
    pub max_awareness: f32,
    pub alert_threshold: f32,
    pub gain_per_second: f32,
    pub loss_per_second: f32,
    pub focused_half_angle_radians: f32,
    pub focused_gain_multiplier: f32,
    pub peripheral_gain_multiplier: f32,
    pub distance_falloff_exponent: f32,
    pub minimum_visibility_factor: f32,
    pub noise_gain_per_second: f32,
    pub forget_after_seconds: f32,
}

impl Default for SpatialAwarenessConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_awareness: 1.0,
            alert_threshold: 0.8,
            gain_per_second: 0.9,
            loss_per_second: 0.45,
            focused_half_angle_radians: 0.24,
            focused_gain_multiplier: 1.35,
            peripheral_gain_multiplier: 0.55,
            distance_falloff_exponent: 1.2,
            minimum_visibility_factor: 0.18,
            noise_gain_per_second: 0.22,
            forget_after_seconds: 8.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SpatialAwarenessEntry {
    pub entity: Entity,
    pub level: AwarenessLevel,
    pub awareness: f32,
    pub visibility_score: f32,
    pub currently_visible: bool,
    pub focused: bool,
    pub last_seen_seconds_ago: Option<f32>,
    pub last_known_position: Option<Vec3>,
}

impl SpatialAwarenessEntry {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            level: AwarenessLevel::Unaware,
            awareness: 0.0,
            visibility_score: 0.0,
            currently_visible: false,
            focused: false,
            last_seen_seconds_ago: None,
            last_known_position: None,
        }
    }

    pub fn is_detected(&self) -> bool {
        self.level == AwarenessLevel::Alert
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SpatialAwarenessScore {
    pub visibility_score: f32,
    pub distance_factor: f32,
    pub focused: bool,
}

pub(crate) fn classify_awareness(
    awareness: f32,
    visible: bool,
    has_seen_target: bool,
    forget_after_seconds: f32,
    last_seen_seconds_ago: Option<f32>,
    alert_threshold: f32,
) -> AwarenessLevel {
    if visible {
        if awareness >= alert_threshold.max(0.0) {
            AwarenessLevel::Alert
        } else {
            AwarenessLevel::Suspicious
        }
    } else if has_seen_target {
        let elapsed = last_seen_seconds_ago.unwrap_or(f32::INFINITY);
        if elapsed > forget_after_seconds.max(0.0) {
            AwarenessLevel::Unaware
        } else if awareness > 0.0 {
            AwarenessLevel::Searching
        } else {
            AwarenessLevel::Lost
        }
    } else {
        AwarenessLevel::Unaware
    }
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
    awareness: &SpatialAwarenessConfig,
) -> SpatialAwarenessScore {
    let range = query.shape.range().max(0.001);
    let distance = sample_distance(query.dimension, query.origin, sample);
    let normalized_distance = 1.0 - (distance / range).clamp(0.0, 1.0);
    let distance_factor = awareness.minimum_visibility_factor.clamp(0.0, 1.0)
        + (1.0 - awareness.minimum_visibility_factor.clamp(0.0, 1.0))
            * normalized_distance.powf(awareness.distance_falloff_exponent.max(0.01));

    let focused = is_focused(query, sample, awareness.focused_half_angle_radians);
    let angular_factor = if focused {
        awareness.focused_gain_multiplier.max(0.0)
    } else {
        awareness.peripheral_gain_multiplier.max(0.0)
    };

    SpatialAwarenessScore {
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
    if matches!(query.shape, SpatialShape::Radius { .. }) {
        return true;
    }

    let SpatialShape::Cone {
        half_angle_radians, ..
    } = query.shape
    else {
        return true;
    };

    let focus_limit = focused_half_angle_radians.min(half_angle_radians.max(0.0));
    let Some(forward) = normalize_direction(query.dimension, query.forward) else {
        return true;
    };
    let Some(direction) = normalize_direction(query.dimension, sample - query.origin) else {
        return true;
    };
    direction.dot(forward) >= focus_limit.cos() - 0.0001
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
#[path = "awareness_tests.rs"]
mod tests;
