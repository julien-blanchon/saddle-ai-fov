use bevy::prelude::*;

use super::{SpatialStimulusConfig, should_forget_target, visibility_score_for_sample};
use crate::spatial::{SpatialDimension, SpatialVisibilityQuery};

#[test]
fn focused_samples_score_higher_than_peripheral_samples() {
    let stimulus = SpatialStimulusConfig::default();
    let query =
        SpatialVisibilityQuery::cone(Vec3::ZERO, Vec3::X, 10.0, 0.6, SpatialDimension::Planar2d);

    let focused = visibility_score_for_sample(&query, Vec3::new(4.0, 0.2, 0.0), &stimulus);
    let peripheral = visibility_score_for_sample(&query, Vec3::new(4.0, 1.8, 0.0), &stimulus);

    assert!(focused.focused);
    assert!(focused.visibility_score > peripheral.visibility_score);
}

#[test]
fn forgetting_respects_timeout() {
    assert!(!should_forget_target(5.0, Some(4.9)));
    assert!(should_forget_target(5.0, Some(5.1)));
    assert!(!should_forget_target(5.0, None));
}
