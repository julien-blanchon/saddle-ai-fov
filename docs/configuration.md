# Configuration

## Runtime

| Type | Field | Default | Effect |
| --- | --- | --- | --- |
| `FovRuntimeConfig` | `max_viewers_per_frame` | `usize::MAX` | Upper bound on dirty viewers processed in one update. Lower this to time-slice large crowds. |
| `FovDebugSettings` | `enabled` | `false` | Master switch for debug drawing. |
| `FovDebugSettings` | `draw_grid_cells` | `true` | Draw currently visible grid cells. |
| `FovDebugSettings` | `draw_view_shapes` | `true` | Draw viewer radii and cone or rect volumes. |
| `FovDebugSettings` | `draw_filled_shapes` | `true` | Fill 2D cones and rects and add inner rings for 3D shapes. |
| `FovDebugSettings` | `draw_occlusion_rays` | `true` | Draw green rays to currently visible targets. |
| `FovDebugSettings` | `draw_blocked_rays` | `false` | Draw dim red rays to in-range targets that are blocked or outside the active shape. |
| `FovDebugSettings` | `draw_occluder_shapes` | `true` | Draw outlines of occluder primitives (discs, rects, spheres, boxes). |
| `FovDebugSettings` | `max_grid_cells_per_viewer` | `96` | Safety cap for grid-cell overlays in busy scenes. |

`FovDebugSettings` affects only the crate-local `FovDebugGizmos` group. Consumers opt into that group with:

```rust,no_run
app.init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>();
```

## Grid Adapter

### `GridMapSpec`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `origin` | `Vec2` | `Vec2::ZERO` | World-space minimum corner of cell `(0, 0)`. |
| `dimensions` | `UVec2` | `32 x 24` | Number of cells in the map. |
| `cell_size` | `Vec2` | `1 x 1` | World-space size of each cell. Non-square cells are supported. |

### `GridOpacityMap`

`GridOpacityMap` stores one `bool` per cell.

- `true`: opaque, blocks grid LOS and shadowcasting
- `false`: transparent

Use `GridOpacityMap::from_fn(...)` to build it from external map data.

### `GridFovConfig`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `radius` | `i32` | `8` | Inclusive circular radius in cells. `0` returns only the origin cell. |
| `backend` | `GridFovBackend` | `RecursiveShadowcasting` | Chooses the grid algorithm. |
| `corner_policy` | `GridCornerPolicy` | `BlockIfBothAdjacentWalls` | Controls diagonal corner-peeking behavior in LOS refinement. |
| `reveal_blockers` | `bool` | `true` | If `true`, an opaque target cell can still count as visible. |

### `GridFov`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `config` | `GridFovConfig` | see above | Per-viewer grid settings. |
| `remember_seen_cells` | `bool` | `true` | If `true`, explored cells persist in `GridFovState::explored`. |
| `enabled` | `bool` | `true` | Disables this viewer without removing the component. |

### `GridFovState`

| Field | Meaning |
| --- | --- |
| `visible_now` | Current visible cells. |
| `explored` | Union of all seen cells when `remember_seen_cells` is on. |
| `entered` | Cells that entered the visible set on the last published change. |
| `exited` | Cells that left the visible set on the last published change. |

## Spatial Adapter

### `VisibilityLayer` and `VisibilityLayerMask`

- `VisibilityLayer(u8)` names one logical visibility layer.
- `VisibilityLayerMask(u64)` lets viewers, targets, and occluders filter each other by overlap.
- Valid layer indices are `0..=63`.

### `SpatialShape`

| Variant | Fields | Effect |
| --- | --- | --- |
| `Radius` | `range` | Omnidirectional range-only visibility (circle in 2D, sphere in 3D). |
| `Cone` | `range`, `half_angle_radians` | Directional cone or sector visibility. |
| `Rect` | `depth`, `half_width`, `half_height` | Axis-aligned rectangular volume oriented along the forward direction. In 2D mode `half_height` is ignored. Useful for cameras, corridor sensors, and rectangular detection zones. |

### `SpatialFov`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `shape` | `SpatialShape` | constructor dependent | Visibility volume. Use constructors: `radius()`, `cone_2d()`, `cone_3d()`, `rect_2d()`, `rect_3d()`. |
| `dimension` | `SpatialDimension` | `Planar2d` | Chooses 2D or full 3D distance/direction tests. |
| `layers` | `VisibilityLayerMask` | `ALL` | Viewer-side layer filter. |
| `local_origin` | `Vec3` | `Vec3::ZERO` | Viewer-local sample point used as the origin of the query. |
| `local_forward` | `Vec3` | `Vec3::X` | Viewer-local forward vector. Used only for directional shapes. |
| `near_override` | `f32` | `0.0` | Distance where targets bypass angular gating. |
| `stimulus` | `SpatialStimulusConfig` | see below | Neutral numeric signal accumulation, decay, and forgetting settings. |
| `remember_seen_targets` | `bool` | `true` | If `true`, previously seen targets remain in `SpatialFovState::remembered`. |
| `enabled` | `bool` | `true` | Disables this viewer without removing the component. |

### `SpatialStimulusConfig`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` | Enables time-based signal accumulation, decay, and forgetting for this viewer. |
| `max_signal` | `f32` | `1.0` | Upper bound for the normalized signal meter. |
| `gain_per_second` | `f32` | `0.9` | Base signal gain per second while the target is directly visible. |
| `loss_per_second` | `f32` | `0.45` | Base signal loss per second while the target is not directly visible. |
| `focused_half_angle_radians` | `f32` | `0.24` | Inner cone treated as focused sensing for directional viewers. |
| `focused_gain_multiplier` | `f32` | `1.35` | Extra gain applied to focused samples. |
| `peripheral_gain_multiplier` | `f32` | `0.55` | Gain applied to samples outside the focused cone but still inside the outer shape. |
| `distance_falloff_exponent` | `f32` | `1.2` | Shapes how strongly direct signal falls off with distance. |
| `minimum_visibility_factor` | `f32` | `0.18` | Floor for distance-based visibility scoring near the edge of range. |
| `indirect_gain_per_second` | `f32` | `0.22` | Base gain used when a target contributes indirect signal while in range and not occluded. |
| `forget_after_seconds` | `f32` | `8.0` | Time after the last direct sighting before a zero-signal target is forgotten. |

### `FovTarget`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `layers` | `VisibilityLayerMask` | `ALL` | Target-side layer filter. |
| `sample_points` | `Vec<Vec3>` | `[Vec3::ZERO]` | Local-space samples tested in order. Multiple samples reduce false negatives for tall or wide targets. |
| `enabled` | `bool` | `true` | Temporarily disables the target. |

### `FovStimulusSource`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `direct_visibility_scale` | `f32` | `1.0` | Multiplies direct-visibility signal gain while the target is visible. |
| `indirect_signal` | `f32` | `0.0` | Adds generic out-of-shape signal while the target is in range and not occluded. Use this for whatever your game considers a secondary cue. |
| `signal_gain_multiplier` | `f32` | `1.0` | Additional target-specific multiplier applied to direct and indirect gain. |
| `signal_loss_multiplier` | `f32` | `1.0` | Additional target-specific multiplier applied to decay while the target is hidden. |

### `FovOccluder`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `layers` | `VisibilityLayerMask` | `ALL` | Occluder-side layer filter. |
| `shape` | `OccluderShape` | constructor dependent | Geometry used by the built-in ECS adapter. |
| `local_offset` | `Vec3` | `Vec3::ZERO` | Offsets the occluder shape relative to the entity transform. |
| `enabled` | `bool` | `true` | Temporarily disables the occluder. |

### `OccluderShape`

| Variant | Fields | Use Case |
| --- | --- | --- |
| `Disc2d` | `radius` | Top-down pillars, round props, simple soft blockers. |
| `Rect2d` | `half_extents` | Top-down walls or crates with entity rotation. |
| `Sphere` | `radius` | Volumetric 3D blockers with simple silhouettes. |
| `Box` | `half_extents` | Volumetric 3D walls, crates, doors, or cover volumes. |

### `SpatialFovState`

| Field | Meaning |
| --- | --- |
| `visible_now` | Targets currently visible to this viewer. |
| `remembered` | Union of all seen targets when memory is enabled. |
| `entered` | Targets that entered visibility on the last published change. |
| `exited` | Targets that left visibility on the last published change. |
| `stimuli` | Per-target neutral signal entries including current signal, visibility score, indirect contribution, and last known position. |

## Optional Stealth Adapter

### `StealthAwarenessConfig`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `alert_threshold` | `f32` | `0.8` | Signal value where the optional stealth adapter promotes a visible target to `Alert`. |

### `StealthAwarenessState`

| Field | Meaning |
| --- | --- |
| `entries` | Per-target stealth levels derived from the core stimulus entries. |
