# Physics Integration

The crate's spatial occlusion layer is callback-based: `evaluate_visibility` takes an
`occluded: impl FnMut(Vec3, Vec3) -> bool` closure. The built-in ECS adapter uses
`occluded_by_any(start, end, dimension, &occluders)` with simple geometric primitives.
When you need collider-accurate raycasts from a physics engine, replace this closure.

## Using Avian3D Raycasts

```rust,no_run
use avian3d::prelude::*;
use bevy::prelude::*;
use saddle_ai_fov::{
    SpatialFov, SpatialVisibilityQuery, FovTarget, evaluate_visibility,
};

fn custom_visibility_check(
    spatial_query: SpatialQuery,
    viewers: Query<(&GlobalTransform, &SpatialFov)>,
    targets: Query<(&GlobalTransform, &FovTarget)>,
) {
    for (viewer_transform, viewer) in &viewers {
        let origin = viewer_transform.transform_point(viewer.local_origin);
        let forward = viewer_transform.transform_point(
            viewer.local_origin + viewer.local_forward
        ) - origin;

        let query = SpatialVisibilityQuery {
            origin,
            forward,
            dimension: viewer.dimension,
            shape: viewer.shape,
            near_override: viewer.near_override,
        };

        for (target_transform, target) in &targets {
            let samples: Vec<Vec3> = target
                .sample_points
                .iter()
                .map(|offset| target_transform.transform_point(*offset))
                .collect();

            let result = evaluate_visibility(&query, &samples, |start, end| {
                // Replace geometric primitives with a physics raycast
                let direction = end - start;
                let max_distance = direction.length();
                if max_distance < f32::EPSILON {
                    return false;
                }
                spatial_query
                    .cast_ray(
                        start,
                        Dir3::new(direction / max_distance).unwrap_or(Dir3::X),
                        max_distance,
                        true,
                        &SpatialQueryFilter::default(),
                    )
                    .is_some()
            });

            if result.visible {
                // target is visible to this viewer through physics geometry
            }
        }
    }
}
```

## Key Points

- The closure receives `(start: Vec3, end: Vec3)` — the ray origin and target sample position.
  Return `true` if the ray is blocked, `false` if clear.
- `evaluate_visibility` handles range checks, cone angle tests, near-override, and
  early-out on the first visible sample. You only provide the occlusion test.
- For 2D games using Avian2D, cast a 2D ray instead and truncate the z coordinate
  from `start` and `end`.
- To combine physics raycasts with the built-in primitives, call `occluded_by_any`
  as a fallback inside the closure.
- Run your custom system in `FovSystems::Recompute` or after it, depending on whether
  you want to supplement or replace the built-in spatial recompute.

## Performance

Physics raycasts are more expensive than geometric primitives. Consider:

- Use `FovRuntimeConfig::max_viewers_per_frame` to budget viewer recomputes.
- Filter targets by layer mask before casting rays.
- Use `SpatialQueryFilter` to exclude irrelevant collider layers.
- For many viewers, batch raycasts or use the budgeting system to spread work across frames.
