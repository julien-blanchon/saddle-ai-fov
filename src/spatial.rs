use std::collections::HashSet;

use bevy::prelude::*;

use crate::components::SpatialFovState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub struct VisibilityLayer(pub u8);

impl VisibilityLayer {
    pub const ZERO: Self = Self(0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub struct VisibilityLayerMask(pub u64);

impl VisibilityLayerMask {
    pub const EMPTY: Self = Self(0);
    pub const ALL: Self = Self(u64::MAX);

    pub fn from_layer(layer: VisibilityLayer) -> Self {
        if layer.0 >= 64 {
            return Self::EMPTY;
        }
        Self(1u64 << layer.0)
    }

    pub fn contains(self, layer: VisibilityLayer) -> bool {
        self.overlaps(Self::from_layer(layer))
    }

    pub fn overlaps(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl Default for VisibilityLayerMask {
    fn default() -> Self {
        Self::ALL
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum SpatialDimension {
    Planar2d,
    Volumetric3d,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum SpatialShape {
    Radius {
        range: f32,
    },
    Cone {
        range: f32,
        half_angle_radians: f32,
    },
    /// Rectangular field of view oriented along the forward direction.
    /// `depth` extends forward from the origin, `half_width` extends left/right
    /// perpendicular to forward.  In 3D mode `half_height` adds vertical extent;
    /// in 2D mode it is ignored.
    Rect {
        depth: f32,
        half_width: f32,
        half_height: f32,
    },
}

impl SpatialShape {
    pub fn range(self) -> f32 {
        match self {
            Self::Radius { range } | Self::Cone { range, .. } => range.max(0.0),
            Self::Rect {
                depth, half_width, ..
            } => {
                // The maximum detection distance is from origin to the far corner.
                (depth * depth + half_width * half_width).sqrt().max(0.0)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct SpatialVisibilityQuery {
    pub origin: Vec3,
    pub forward: Vec3,
    pub dimension: SpatialDimension,
    pub shape: SpatialShape,
    pub near_override: f32,
}

impl SpatialVisibilityQuery {
    pub fn radius(origin: Vec3, range: f32, dimension: SpatialDimension) -> Self {
        Self {
            origin,
            forward: Vec3::X,
            dimension,
            shape: SpatialShape::Radius {
                range: range.max(0.0),
            },
            near_override: 0.0,
        }
    }

    pub fn cone(
        origin: Vec3,
        forward: Vec3,
        range: f32,
        half_angle_radians: f32,
        dimension: SpatialDimension,
    ) -> Self {
        Self {
            origin,
            forward,
            dimension,
            shape: SpatialShape::Cone {
                range: range.max(0.0),
                half_angle_radians: half_angle_radians.max(0.0),
            },
            near_override: 0.0,
        }
    }

    pub fn rect(
        origin: Vec3,
        forward: Vec3,
        depth: f32,
        half_width: f32,
        half_height: f32,
        dimension: SpatialDimension,
    ) -> Self {
        Self {
            origin,
            forward,
            dimension,
            shape: SpatialShape::Rect {
                depth: depth.max(0.0),
                half_width: half_width.max(0.0),
                half_height: half_height.max(0.0),
            },
            near_override: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum OccluderShape {
    Disc2d { radius: f32 },
    Rect2d { half_extents: Vec2 },
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct WorldOccluder {
    pub shape: OccluderShape,
    pub translation: Vec3,
    pub rotation: Quat,
}

#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct VisibilityTestResult {
    pub in_range: bool,
    pub inside_shape: bool,
    pub occluded: bool,
    pub visible: bool,
    pub visible_sample_index: Option<usize>,
    pub visible_sample: Option<Vec3>,
    pub checked_samples: usize,
    pub rays_cast: usize,
}

pub fn evaluate_visibility(
    query: &SpatialVisibilityQuery,
    target_samples: &[Vec3],
    mut occluded: impl FnMut(Vec3, Vec3) -> bool,
) -> VisibilityTestResult {
    let mut result = VisibilityTestResult::default();

    for (index, sample) in target_samples.iter().copied().enumerate() {
        result.checked_samples += 1;

        let distance = sample_distance(query.dimension, query.origin, sample);
        if distance > query.shape.range() {
            continue;
        }

        result.in_range = true;

        if !sample_inside_shape(query, sample, distance) {
            continue;
        }

        result.inside_shape = true;
        result.rays_cast += 1;

        if occluded(query.origin, sample) {
            result.occluded = true;
            continue;
        }

        result.visible = true;
        result.visible_sample_index = Some(index);
        result.visible_sample = Some(sample);
        break;
    }

    result
}

pub fn occluded_by_any(
    start: Vec3,
    end: Vec3,
    dimension: SpatialDimension,
    occluders: &[WorldOccluder],
) -> bool {
    occluders
        .iter()
        .any(|occluder| segment_hits_occluder(start, end, dimension, *occluder))
}

pub fn merge_spatial_visibility<'a>(
    states: impl IntoIterator<Item = &'a SpatialFovState>,
) -> Vec<Entity> {
    let mut merged = HashSet::new();
    for state in states {
        merged.extend(state.visible_now.iter().copied());
    }

    let mut merged_vec: Vec<_> = merged.into_iter().collect();
    merged_vec.sort_by_key(entity_sort_key);
    merged_vec
}

fn sample_inside_shape(query: &SpatialVisibilityQuery, sample: Vec3, distance: f32) -> bool {
    if distance <= query.near_override.max(0.0) {
        return true;
    }

    match query.shape {
        SpatialShape::Radius { .. } => true,
        SpatialShape::Cone {
            half_angle_radians, ..
        } => {
            let Some(forward) = normalize_direction(query.dimension, query.forward) else {
                return true;
            };

            let Some(direction) = normalize_direction(query.dimension, sample - query.origin)
            else {
                return true;
            };

            direction.dot(forward) >= half_angle_radians.cos() - 0.0001
        }
        SpatialShape::Rect {
            depth,
            half_width,
            half_height,
        } => {
            let Some(forward) = normalize_direction(query.dimension, query.forward) else {
                return true;
            };

            let offset = sample - query.origin;

            // Project onto local axes.  "forward" is depth, "right" is perpendicular.
            let along = offset.dot(forward);
            if along < 0.0 || along > depth.max(0.0) {
                return false;
            }

            match query.dimension {
                SpatialDimension::Planar2d => {
                    let right = Vec3::new(-forward.y, forward.x, 0.0);
                    let lateral = offset.dot(right).abs();
                    lateral <= half_width.max(0.0)
                }
                SpatialDimension::Volumetric3d => {
                    // Build a local right/up basis from forward.
                    let up_hint = if forward.cross(Vec3::Y).length_squared() > 0.0001 {
                        Vec3::Y
                    } else {
                        Vec3::X
                    };
                    let right = forward.cross(up_hint).normalize();
                    let up = right.cross(forward).normalize();
                    let lateral = offset.dot(right).abs();
                    let vertical = offset.dot(up).abs();
                    lateral <= half_width.max(0.0) && vertical <= half_height.max(0.0)
                }
            }
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

fn segment_hits_occluder(
    start: Vec3,
    end: Vec3,
    dimension: SpatialDimension,
    occluder: WorldOccluder,
) -> bool {
    match occluder.shape {
        OccluderShape::Disc2d { radius } => {
            if dimension != SpatialDimension::Planar2d {
                return false;
            }
            segment_distance_sq_2d(
                start.truncate(),
                end.truncate(),
                occluder.translation.truncate(),
            ) <= radius * radius
        }
        OccluderShape::Rect2d { half_extents } => {
            if dimension != SpatialDimension::Planar2d {
                return false;
            }
            let inv = occluder.rotation.inverse();
            let local_start = inv * (start - occluder.translation);
            let local_end = inv * (end - occluder.translation);
            segment_intersects_aabb_2d(local_start.truncate(), local_end.truncate(), half_extents)
        }
        OccluderShape::Sphere { radius } => {
            segment_distance_sq_3d(start, end, occluder.translation) <= radius * radius
        }
        OccluderShape::Box { half_extents } => {
            let inv = occluder.rotation.inverse();
            let local_start = inv * (start - occluder.translation);
            let local_end = inv * (end - occluder.translation);
            segment_intersects_aabb_3d(local_start, local_end, half_extents)
        }
    }
}

fn segment_distance_sq_2d(start: Vec2, end: Vec2, point: Vec2) -> f32 {
    let delta = end - start;
    let len_sq = delta.length_squared();
    if len_sq <= f32::EPSILON {
        return start.distance_squared(point);
    }

    let t = ((point - start).dot(delta) / len_sq).clamp(0.0, 1.0);
    (start + delta * t).distance_squared(point)
}

fn segment_distance_sq_3d(start: Vec3, end: Vec3, point: Vec3) -> f32 {
    let delta = end - start;
    let len_sq = delta.length_squared();
    if len_sq <= f32::EPSILON {
        return start.distance_squared(point);
    }

    let t = ((point - start).dot(delta) / len_sq).clamp(0.0, 1.0);
    (start + delta * t).distance_squared(point)
}

fn segment_intersects_aabb_2d(start: Vec2, end: Vec2, half_extents: Vec2) -> bool {
    let delta = end - start;
    let mut t_min: f32 = 0.0;
    let mut t_max: f32 = 1.0;

    for axis in 0..2 {
        let origin = if axis == 0 { start.x } else { start.y };
        let direction = if axis == 0 { delta.x } else { delta.y };
        let min = if axis == 0 {
            -half_extents.x
        } else {
            -half_extents.y
        };
        let max = if axis == 0 {
            half_extents.x
        } else {
            half_extents.y
        };

        if direction.abs() <= f32::EPSILON {
            if origin < min || origin > max {
                return false;
            }
            continue;
        }

        let inv = 1.0 / direction;
        let mut t1 = (min - origin) * inv;
        let mut t2 = (max - origin) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_min > t_max {
            return false;
        }
    }

    true
}

fn segment_intersects_aabb_3d(start: Vec3, end: Vec3, half_extents: Vec3) -> bool {
    let delta = end - start;
    let mut t_min: f32 = 0.0;
    let mut t_max: f32 = 1.0;

    for axis in 0..3 {
        let origin = match axis {
            0 => start.x,
            1 => start.y,
            _ => start.z,
        };
        let direction = match axis {
            0 => delta.x,
            1 => delta.y,
            _ => delta.z,
        };
        let min = match axis {
            0 => -half_extents.x,
            1 => -half_extents.y,
            _ => -half_extents.z,
        };
        let max = match axis {
            0 => half_extents.x,
            1 => half_extents.y,
            _ => half_extents.z,
        };

        if direction.abs() <= f32::EPSILON {
            if origin < min || origin > max {
                return false;
            }
            continue;
        }

        let inv = 1.0 / direction;
        let mut t1 = (min - origin) * inv;
        let mut t2 = (max - origin) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_min > t_max {
            return false;
        }
    }

    true
}

fn entity_sort_key(entity: &Entity) -> (u32, u32) {
    (entity.index().index(), entity.generation().to_bits())
}

#[cfg(test)]
#[path = "spatial_tests.rs"]
mod tests;
