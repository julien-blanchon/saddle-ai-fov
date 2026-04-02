# Architecture

## Layering

`saddle-ai-fov` is split into three layers:

1. Pure visibility logic
   - `src/algorithms/shadowcasting.rs`
   - `src/algorithms/los.rs`
   - `src/grid.rs`
   - `src/spatial.rs`
2. ECS adapter layer
   - `src/components.rs`
   - `src/resources.rs`
   - `src/systems.rs`
3. Debug and showcase layer
   - `src/debug.rs`
   - `examples/`
   - `examples/lab/`

The pure layer owns correctness. The ECS layer only adapts world data into pure queries and publishes stable component state for downstream systems.

## Runtime Flow

```text
GridOpacityMap / FovTarget / FovOccluder / viewer components
  -> Prepare: ensure state components exist
  -> MarkDirty: watch changed transforms, config, targets, occluders, and grid resources
  -> Recompute: process dirty viewers up to the configured budget
  -> publish GridFovState / SpatialFovState only when the visible sets actually change
  -> DebugDraw: visualize cells, radii, cones, and visible rays when enabled
```

## Dirty Propagation

The runtime uses an explicit `FovDirty` marker.

Viewers are marked dirty when:

- the viewer is added
- `GridFov` or `SpatialFov` changes
- the viewer transform changes
- the grid opacity map changes
- a target or occluder changes
- a target or occluder is removed

Dirty viewers stay dirty until the recompute system actually processes them. That is what makes per-frame budgets safe.

## Budgeting

`FovRuntimeConfig::max_viewers_per_frame` caps how many dirty viewers are recomputed in one update.

Scaling notes:

- Grid FOV cost scales roughly with `viewers * visible area`.
- `RecursiveShadowcasting` visits only the octants inside the configured radius, then refines with LOS where needed.
- `RaycastLos` scales more directly with `viewers * radius² * line length`.
- Spatial visibility cost scales with `viewers * candidate targets * target samples`.
- Occlusion cost scales with `visible target samples * relevant occluders`.

The built-in stats resource publishes:

- `dirty_viewers`
- `recomputed_viewers`
- `visible_cells_total`
- `visible_targets_total`
- `target_checks`
- `occlusion_tests`
- `last_recompute_micros`

## State Semantics

`GridFovState` and `SpatialFovState` are intended to be `Changed<T>`-friendly.

They publish:

- the current visible set
- the remembered or explored set
- the latest `entered`
- the latest `exited`

They do **not** update on every recompute if the visible sets stayed the same. This keeps downstream reaction systems stable.

## Grid Adapter

`GridOpacityMap` is a simple square-grid opacity resource:

- `GridMapSpec` maps between world space and grid space
- cells are stored as a flat `Vec<bool>`
- out-of-bounds cells are treated as opaque by LOS and shadowcasting

This is intentionally simple. Consumers can populate it from any tilemap or procedural map source they want.

## Spatial Adapter

The pure spatial layer is callback-based:

- `SpatialVisibilityQuery` describes the viewer
- `evaluate_visibility` decides whether any sample is visible
- the caller supplies the occlusion test

The ECS layer ships a default adapter based on `FovOccluder`:

- 2D discs
- 2D axis-aligned rectangles transformed by entity rotation
- 3D spheres
- 3D boxes

That keeps the shared crate useful in projects without a physics dependency while still leaving room for engine- or physics-backed traces.

## Transform Caveat

The runtime queries `GlobalTransform` for world-space visibility. If your viewer or targets are in a hierarchy and you need same-frame child transform propagation, run the plugin in a schedule where `GlobalTransform` is already authoritative, or update the transforms before this runtime in a way that fits your app.

The crate-local examples keep their animated entities as roots and update `Transform` and `GlobalTransform` together so the showcase remains deterministic.

## Verification Strategy

The crate verifies each layer separately:

- unit tests for exact grid visibility, LOS corner cases, occlusion primitives, and dirty/budget behavior
- standalone examples for grid memory, multi-viewer merging, 2D cones, and 3D cones
- a crate-local lab with BRP support and E2E scenarios for smoke, exploration memory, and cone occlusion

## Debug Gizmos

The debug renderer uses the dedicated `FovDebugGizmos` config group instead of the default gizmo group.

That keeps the crate's overlays independently toggleable:

- normal consumers can ignore the group and keep debug rendering off
- examples and labs can opt in with `app.init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>()`
- BRP inspection can read `FovDebugSettings` without mixing the crate's debug lines into unrelated gizmo categories
