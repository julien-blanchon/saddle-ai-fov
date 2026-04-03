use bevy::prelude::*;

use super::{
    AwarenessLevel, SpatialAwarenessConfig, classify_awareness, should_forget_target,
    visibility_score_for_sample,
};
use crate::spatial::{SpatialDimension, SpatialVisibilityQuery};

#[test]
fn focused_samples_score_higher_than_peripheral_samples() {
    let awareness = SpatialAwarenessConfig::default();
    let query =
        SpatialVisibilityQuery::cone(Vec3::ZERO, Vec3::X, 10.0, 0.6, SpatialDimension::Planar2d);

    let focused = visibility_score_for_sample(&query, Vec3::new(4.0, 0.2, 0.0), &awareness);
    let peripheral = visibility_score_for_sample(&query, Vec3::new(4.0, 1.8, 0.0), &awareness);

    assert!(focused.focused);
    assert!(focused.visibility_score > peripheral.visibility_score);
}

#[test]
fn awareness_state_walks_detection_pipeline() {
    let config = SpatialAwarenessConfig::default();

    assert_eq!(
        classify_awareness(
            0.0,
            false,
            false,
            config.forget_after_seconds,
            None,
            config.alert_threshold
        ),
        AwarenessLevel::Unaware
    );
    assert_eq!(
        classify_awareness(
            0.3,
            true,
            true,
            config.forget_after_seconds,
            Some(0.0),
            config.alert_threshold
        ),
        AwarenessLevel::Suspicious
    );
    assert_eq!(
        classify_awareness(
            0.95,
            true,
            true,
            config.forget_after_seconds,
            Some(0.0),
            config.alert_threshold
        ),
        AwarenessLevel::Alert
    );
    assert_eq!(
        classify_awareness(
            0.4,
            false,
            true,
            config.forget_after_seconds,
            Some(1.2),
            config.alert_threshold
        ),
        AwarenessLevel::Searching
    );
    assert_eq!(
        classify_awareness(
            0.0,
            false,
            true,
            config.forget_after_seconds,
            Some(2.2),
            config.alert_threshold
        ),
        AwarenessLevel::Lost
    );
    assert_eq!(
        classify_awareness(
            0.0,
            false,
            true,
            config.forget_after_seconds,
            Some(config.forget_after_seconds + 0.1),
            config.alert_threshold
        ),
        AwarenessLevel::Unaware
    );
}

#[test]
fn forgetting_respects_timeout() {
    assert!(!should_forget_target(5.0, Some(4.9)));
    assert!(should_forget_target(5.0, Some(5.1)));
    assert!(!should_forget_target(5.0, None));
}
