# Saddle AI FOV

Reusable field-of-view, occlusion, and stimulus toolkit for Bevy.

The crate stays project-agnostic. It does not assume a tilemap crate, a physics backend, or a stealth-specific state machine. The core layer owns raw visibility and a neutral numeric stimulus model; game-specific interpretations such as stealth awareness are layered on top through optional plugins or your own systems.

## Quick Start

```toml
[dependencies]
saddle-ai-fov = { git = "https://github.com/julien-blanchon/saddle-ai-fov" }
```

```rust,no_run
use bevy::prelude::*;
use saddle_ai_fov::{FovPlugin, GridFov, GridOpacityMap, SpatialFov, SpatialStimulusConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(GridOpacityMap::default())
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Grid Viewer"),
        GridFov::new(6),
        Transform::from_xyz(4.5, 5.5, 0.0),
    ));

    commands.spawn((
        Name::new("Cone Sensor"),
        SpatialFov::cone_2d(320.0, 0.55).with_stimulus(SpatialStimulusConfig::default()),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}
```

If you want the built-in debug overlays, initialize the crate gizmo group in an app that already uses Bevy rendering:

```rust,no_run
app.init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>();
```

Pure-Rust consumers can skip ECS entirely and call `compute_grid_fov(...)` or `evaluate_visibility(...)` directly.

## Perception Pipeline

The spatial pipeline is intentionally split into stages:

1. Raw visibility and occlusion
   - `SpatialFovState::{visible_now, remembered, entered, exited}`
   - `SpatialVisibilityChanged`
2. Neutral numeric stimulus
   - `SpatialStimulusConfig`
   - `FovStimulusSource`
   - `SpatialFovState::stimuli`
   - `SpatialStimulusChanged`
3. Optional game-specific interpretation
   - `StealthAwarenessPlugin`
   - `StealthAwarenessState`
   - `StealthAwarenessChanged`, `StealthTargetDetected`, `StealthTargetLost`

If you want LOS-only gameplay, ignore stage 2 and 3. If you want a tactical or stealth layer, consume the stage 1 and 2 outputs in your own systems or use the optional stealth adapter as a starting point.

## Public API

- Core plugin and sets:
  `FovPlugin`, `FovSystems::{Prepare, MarkDirty, Recompute, DebugDraw}`
- Runtime resources:
  `FovRuntimeConfig`, `FovStats`, `FovDebugSettings`, `FovDebugGizmos`
- Grid layer:
  `GridMapSpec`, `GridOpacityMap`, `GridFovConfig`, `GridFovBackend`, `GridCornerPolicy`, `GridFovResult`, `compute_grid_fov`
- Spatial layer:
  `SpatialVisibilityQuery`, `SpatialShape` (`Radius`, `Cone`, `Rect`), `SpatialDimension`, `VisibilityLayer`, `VisibilityLayerMask`, `VisibilityTestResult`
- ECS viewer components:
  `GridFov`, `GridFovState`, `SpatialFov`, `SpatialFovState`, `FovDirty`
- ECS world inputs:
  `FovTarget`, `FovOccluder`, `OccluderShape`, `FovStimulusSource`
- Core perception outputs:
  `SpatialStimulusConfig`, `SpatialStimulusEntry`, `SpatialVisibilityChanged`, `SpatialStimulusChanged`
- Optional stealth adapter:
  `StealthAwarenessPlugin`, `StealthAwarenessSystems`, `StealthAwarenessConfig`, `StealthAwarenessState`, `StealthAwarenessEntry`, `StealthAwarenessLevel`
- Pure helpers:
  `evaluate_visibility`, `occluded_by_any`, `has_grid_line_of_sight`, `supercover_line`, `merge_grid_visibility`, `merge_spatial_visibility`

## Behavior Notes

- `GridCornerPolicy::IgnoreAdjacentWalls` allows diagonal peeking through a one-cell pinch.
- `GridCornerPolicy::BlockIfBothAdjacentWalls` blocks only when both side-adjacent cells at a corner crossing are opaque.
- `GridCornerPolicy::BlockIfEitherAdjacentWall` is the strictest option and blocks on either side-adjacent wall.
- `GridFovConfig::reveal_blockers` controls whether opaque target cells count as visible.
- A cone with a zero-length forward vector degrades to range-only behavior instead of panicking.
- `SpatialFov::with_near_override(...)` lets very close targets bypass angular gating, which keeps perception stable near cone boundaries.
- Stimulus-enabled spatial viewers continue recomputing while active so numeric signals can rise, decay, and forget on time alone.
- `FovStimulusSource` is intentionally generic: direct visibility scale, indirect signal, and gain/loss multipliers can stand in for lighting, sound, thermal signatures, radio pings, or other project-specific concepts.
- The core crate does not hardcode stealth progression. The bundled stealth adapter is optional and layered on top of the neutral outputs.

## Examples

Run examples from the examples workspace:

```bash
cd examples
```

| Example | Purpose | Run |
| --- | --- | --- |
| `basic_grid` | Minimal square-grid FOV with one moving viewer | `cargo run -p saddle-ai-fov-example-basic-grid` |
| `exploration_memory` | Demonstrates `visible_now` plus persistent explored cells | `cargo run -p saddle-ai-fov-example-exploration-memory` |
| `multi_viewers` | Merges multiple viewer states without coupling the viewers together | `cargo run -p saddle-ai-fov-example-multi-viewers` |
| `cone_2d` | Top-down cone checks with simple occluder primitives and live debug rays | `cargo run -p saddle-ai-fov-example-cone-2d` |
| `cone_3d` | 3D sentry vision with multi-sample targets and box occluders | `cargo run -p saddle-ai-fov-example-cone-3d` |
| `radius_2d` | Omnidirectional 2D detection radius with orbiting targets and occluders | `cargo run -p saddle-ai-fov-example-radius-2d` |
| `rect_2d` | Rectangular FOV for cameras and corridor sensors with depth/width controls | `cargo run -p saddle-ai-fov-example-rect-2d` |
| `layers` | Visibility layer filtering with multiple independent viewers | `cargo run -p saddle-ai-fov-example-layers` |
| `perception_kernel` | Neutral stimulus demo: raw visibility plus a numeric signal bar, no state machine | `cargo run -p saddle-ai-fov-example-perception-kernel` |
| `stealth_detection` | Concrete stealth adapter built on top of the core stimulus kernel | `cargo run -p saddle-ai-fov-example-stealth-detection` |
| `saddle-ai-fov-lab` | Crate-local verification app with BRP and E2E hooks | `cargo run -p saddle-ai-fov-lab` |

## Crate-Local Lab

`examples/lab` is the verification app for this crate. The left side exercises grid memory. The right side shows the full spatial pipeline: visibility, neutral stimulus, and optional stealth mapping.

```bash
cd examples
cargo run -p saddle-ai-fov-lab
```

E2E commands:

```bash
cd examples
cargo run -p saddle-ai-fov-lab --features e2e -- smoke_launch
cargo run -p saddle-ai-fov-lab --features e2e -- fov_grid_memory
cargo run -p saddle-ai-fov-lab --features e2e -- fov_cone_occlusion
cargo run -p saddle-ai-fov-lab --features e2e -- fov_stimulus_pipeline
```

`fov_smoke` remains as a backward-compatible alias for `smoke_launch`, and `fov_awareness_detection` remains as a backward-compatible alias for `fov_stimulus_pipeline`.

## BRP

Useful BRP commands against the lab:

```bash
uv run --project .codex/skills/bevy-brp/script brp app launch saddle-ai-fov-lab
uv run --project .codex/skills/bevy-brp/script brp world query bevy_ecs::name::Name
uv run --project .codex/skills/bevy-brp/script brp world query saddle_ai_fov::components::GridFovState
uv run --project .codex/skills/bevy-brp/script brp world query saddle_ai_fov::components::SpatialFovState
uv run --project .codex/skills/bevy-brp/script brp world query saddle_ai_fov::StealthAwarenessState
uv run --project .codex/skills/bevy-brp/script brp resource get saddle_ai_fov::resources::FovStats
uv run --project .codex/skills/bevy-brp/script brp resource get saddle_ai_fov::debug::FovDebugSettings
uv run --project .codex/skills/bevy-brp/script brp extras screenshot /tmp/fov_lab.png
uv run --project .codex/skills/bevy-brp/script brp extras shutdown
```

## Limitations

- The built-in grid adapter is square-grid only. Hex support is not included in v1.
- `RecursiveShadowcasting` is the default backend, not a fully symmetric shadowcasting implementation.
- The ECS spatial occlusion layer uses simple discs, rectangles, spheres, and boxes. If you need collider-accurate 3D traces, call `evaluate_visibility` with your own adapter.
- The core crate owns visibility and neutral numeric stimulus only. It does not ship a game-specific stealth, tactical, or squad-state workflow by default.

## More Docs

- [Architecture](docs/architecture.md)
- [Configuration](docs/configuration.md)
- [Algorithm Tradeoffs](docs/algorithm-tradeoffs.md)
- [Physics Integration](docs/physics-integration.md)
