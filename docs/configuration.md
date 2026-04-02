# Configuration

## Runtime

| Type | Field | Default | Effect |
| --- | --- | --- | --- |
| `FovRuntimeConfig` | `max_viewers_per_frame` | `usize::MAX` | Upper bound on dirty viewers processed in one update. Lower this to time-slice large crowds. |
| `FovDebugSettings` | `enabled` | `false` | Master switch for debug drawing. |
| `FovDebugSettings` | `draw_grid_cells` | `true` | Draw currently visible grid cells. |
| `FovDebugSettings` | `draw_view_shapes` | `true` | Draw viewer radii and cone volumes. |
| `FovDebugSettings` | `draw_occlusion_rays` | `true` | Draw rays to currently visible targets. |
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
| `Radius` | `range` | Range-only visibility. |
| `Cone` | `range`, `half_angle_radians` | Directional visibility. |

### `SpatialFov`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `shape` | `SpatialShape` | `Radius` or `Cone` constructor dependent | Visibility volume. |
| `dimension` | `SpatialDimension` | `Planar2d` | Chooses 2D or full 3D distance/direction tests. |
| `layers` | `VisibilityLayerMask` | `ALL` | Viewer-side layer filter. |
| `local_origin` | `Vec3` | `Vec3::ZERO` | Viewer-local sample point used as the origin of the query. |
| `local_forward` | `Vec3` | `Vec3::X` | Viewer-local forward vector. Used only for cone tests. |
| `near_override` | `f32` | `0.0` | Distance where targets bypass angular gating. |
| `remember_seen_targets` | `bool` | `true` | If `true`, previously seen targets stay in `SpatialFovState::remembered`. |
| `enabled` | `bool` | `true` | Disables this viewer without removing the component. |

### `FovTarget`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `layers` | `VisibilityLayerMask` | `ALL` | Target-side layer filter. |
| `sample_points` | `Vec<Vec3>` | `[Vec3::ZERO]` | Local-space samples tested in order. Multiple samples reduce false negatives for tall or wide targets. |
| `enabled` | `bool` | `true` | Temporarily disables the target. |

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
