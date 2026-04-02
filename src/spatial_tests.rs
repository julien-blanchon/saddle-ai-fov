use bevy::prelude::*;

use super::{
    OccluderShape, SpatialDimension, SpatialVisibilityQuery, VisibilityTestResult, WorldOccluder,
    evaluate_visibility, occluded_by_any,
};

fn lab_query() -> SpatialVisibilityQuery {
    SpatialVisibilityQuery::cone(
        Vec3::new(180.0, 0.0, 0.0),
        Vec3::X,
        420.0,
        0.56,
        SpatialDimension::Planar2d,
    )
}

fn lab_occluders() -> [WorldOccluder; 1] {
    [WorldOccluder {
        shape: OccluderShape::Rect2d {
            half_extents: Vec2::new(24.0, 110.0),
        },
        translation: Vec3::new(420.0, 30.0, 0.0),
        rotation: Quat::IDENTITY,
    }]
}

fn visibility_for(sample: Vec3) -> VisibilityTestResult {
    evaluate_visibility(&lab_query(), &[sample], |start, end| {
        occluded_by_any(start, end, SpatialDimension::Planar2d, &lab_occluders())
    })
}

#[test]
fn lab_front_target_stays_visible_below_the_occluder() {
    let result = visibility_for(Vec3::new(570.0, -150.0, 0.0));

    assert!(result.in_range);
    assert!(result.inside_shape);
    assert!(!result.occluded);
    assert!(result.visible);
}

#[test]
fn lab_hidden_target_is_blocked_by_the_occluder() {
    let result = visibility_for(Vec3::new(565.0, 120.0, 0.0));

    assert!(result.in_range);
    assert!(result.inside_shape);
    assert!(result.occluded);
    assert!(!result.visible);
}

#[test]
fn lab_off_angle_target_stays_outside_the_cone() {
    let result = visibility_for(Vec3::new(320.0, 210.0, 0.0));

    assert!(result.in_range);
    assert!(!result.inside_shape);
    assert!(!result.visible);
    assert_eq!(result.rays_cast, 0);
}

#[test]
fn zero_length_forward_degrades_to_range_only() {
    let result = evaluate_visibility(
        &SpatialVisibilityQuery::cone(Vec3::ZERO, Vec3::ZERO, 6.0, 0.2, SpatialDimension::Planar2d),
        &[Vec3::new(0.0, 4.0, 0.0)],
        |_, _| false,
    );

    assert!(result.in_range);
    assert!(result.inside_shape);
    assert!(result.visible);
}

#[test]
fn near_override_keeps_close_target_visible_even_outside_the_cone() {
    let mut query =
        SpatialVisibilityQuery::cone(Vec3::ZERO, Vec3::X, 10.0, 0.15, SpatialDimension::Planar2d);
    query.near_override = 3.0;

    let result = evaluate_visibility(&query, &[Vec3::new(1.0, 2.0, 0.0)], |_, _| false);

    assert!(result.in_range);
    assert!(result.inside_shape);
    assert!(result.visible);
}

#[test]
fn multi_sample_target_is_visible_when_any_sample_is_clear() {
    let result = evaluate_visibility(
        &SpatialVisibilityQuery::cone(Vec3::ZERO, Vec3::X, 10.0, 0.6, SpatialDimension::Planar2d),
        &[Vec3::new(4.0, 1.2, 0.0), Vec3::new(4.0, -0.4, 0.0)],
        |_, end| end.y > 0.0,
    );

    assert!(result.visible);
    assert_eq!(result.visible_sample_index, Some(1));
    assert_eq!(result.rays_cast, 2);
}
