# Architecture

## Layering

`saddle-ai-fov` is split into four layers:

1. Pure visibility logic
   - `src/algorithms/shadowcasting.rs`
   - `src/algorithms/los.rs`
   - `src/grid.rs`
   - `src/spatial.rs`
2. ECS visibility adapter
   - `src/components.rs`
   - `src/resources.rs`
   - `src/systems.rs`
3. Neutral stimulus kernel
   - `src/stimulus.rs`
   - `SpatialFovState::stimuli`
   - `SpatialStimulusChanged`
4. Optional game-specific adapters
   - `src/stealth.rs`
   - `examples/`
   - `examples/lab/`

The core layer owns correctness. The ECS layer adapts world data into pure queries, publishes stable visibility state, and optionally accumulates a neutral numeric signal. Project-specific meaning is intentionally pushed out into optional layers.

## Runtime Flow

```text
GridOpacityMap / FovTarget / FovOccluder / viewer components
  -> Prepare: ensure state components exist
  -> MarkDirty: watch changed transforms, config, targets, occluders, and grid resources
  -> Recompute: process dirty viewers up to the configured budget
  -> publish raw GridFovState / SpatialFovState visibility
  -> accumulate or decay per-target neutral stimulus values
  -> emit GridVisibilityChanged / SpatialVisibilityChanged / SpatialStimulusChanged
  -> optional adapters map those signals into stealth, tactical, or custom game state
  -> DebugDraw: visualize cells, radii, cones, rects, and visible rays when enabled
```

## Dirty Propagation

The runtime uses an explicit `FovDirty` marker.

Viewers are marked dirty when:

- the viewer is added
- `GridFov` or `SpatialFov` changes
- the viewer transform changes
- the grid opacity map changes
- a target, stimulus source, or occluder changes
- a target, stimulus source, or occluder is removed

Dirty viewers stay dirty until the recompute system actually processes them. That is what makes per-frame budgets safe.

One important exception exists for stimulus-enabled spatial viewers: they are also marked dirty every update so their numeric signal can rise, decay, and forget on time alone.

## Budgeting

`FovRuntimeConfig::max_viewers_per_frame` caps how many dirty viewers are recomputed in one update.

Scaling notes:

- Grid FOV cost scales roughly with `viewers * visible area`.
- `RecursiveShadowcasting` visits only the octants inside the configured radius, then refines with LOS where needed.
- `RaycastLos` scales more directly with `viewers * radius² * line length`.
- Spatial visibility cost scales with `viewers * candidate targets * target samples`.
- Occlusion cost scales with `visible target samples * relevant occluders`.
- Stimulus cost is effectively layered on top of spatial visibility; it reuses the same visibility results instead of running a second world query.

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
- per-target neutral stimulus entries for spatial viewers

They do not update on every recompute if the published state stayed the same. That keeps downstream reaction systems stable.

The important design boundary is this:

- `SpatialFovState` says what is visible and what numeric signal exists
- optional adapters decide what that means for gameplay

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

## Stimulus Kernel

The core stimulus model is deliberately generic.

It combines:

- direct visibility scoring from distance and focused versus peripheral sensing
- per-target direct visibility scaling
- optional indirect signal while the target is in range and not occluded
- time-based decay and forgetting

The result is a normalized signal value with no baked-in stealth semantics. A game can treat that value as suspicion, threat, scan confidence, recon quality, audio interest, or ignore it entirely.

## Optional Stealth Adapter

`StealthAwarenessPlugin` is provided as an example of how to layer project-specific meaning on top of the core stimulus output.

It reads `SpatialFovState::stimuli`, maps those entries into `StealthAwarenessLevel`, and emits stealth-specific messages. It is intentionally separate from `FovPlugin`, so consumers can replace it with their own logic without editing library internals.

## Transform Caveat

The runtime queries `GlobalTransform` for world-space visibility. If your viewer or targets are in a hierarchy and you need same-frame child transform propagation, run the plugin in a schedule where `GlobalTransform` is already authoritative, or update the transforms before this runtime in a way that fits your app.

The crate-local examples keep their animated entities as roots and update `Transform` and `GlobalTransform` together so the showcase remains deterministic.

## Verification Strategy

The crate verifies each layer separately:

- unit tests for exact grid visibility, LOS corner cases, occlusion primitives, stimulus behavior, and dirty or budget behavior
- standalone examples for grid memory, multi-viewer merging, 2D cones, 3D cones, the raw stimulus kernel, and the optional stealth adapter
- a crate-local lab with BRP support and E2E scenarios for smoke, exploration memory, cone occlusion, and the stimulus-to-stealth pipeline

## Debug Gizmos

The debug renderer uses the dedicated `FovDebugGizmos` config group instead of the default gizmo group.

That keeps the crate overlays independently toggleable:

- normal consumers can ignore the group and keep debug rendering off
- examples and labs can opt in with `app.init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>()`
- BRP inspection can read `FovDebugSettings` without mixing the crate overlays into unrelated gizmo categories
