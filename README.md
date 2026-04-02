# Saddle AI FOV

Reusable field-of-view and line-of-sight toolkit for Bevy.

The crate stays project-agnostic. It does not import `game_core`, it does not assume a tilemap crate, and it does not hide a physics dependency behind its 3D visibility checks. The core layer is plain Rust: square-grid FOV, supercover grid LOS, cone/range tests, and occlusion helpers built around simple geometry and callback-based queries.

## Quick Start

```toml
[dependencies]
saddle-ai-fov = { git = "https://github.com/julien-blanchon/saddle-ai-fov" }
```

```rust,no_run
use bevy::prelude::*;
use saddle_ai_fov::{FovPlugin, GridFov, GridOpacityMap};

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DemoState {
    #[default]
    Active,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<DemoState>()
        .insert_resource(GridOpacityMap::default())
        .add_plugins(FovPlugin::new(
            OnEnter(DemoState::Active),
            OnExit(DemoState::Active),
            Update,
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Viewer"),
        GridFov::new(6),
        Transform::from_xyz(4.5, 5.5, 0.0),
    ));
}
```

For apps where visibility should remain active for the full app lifetime, `FovPlugin::default()` is the always-on entrypoint.

If you want the built-in debug overlays, initialize the crate's gizmo group in an app that already uses Bevy's normal rendering plugins:

```rust,no_run
app.init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>();
```

Pure-Rust consumers can skip ECS entirely and call `compute_grid_fov(...)` or `evaluate_visibility(...)` directly.

## Public API

- Plugin: `FovPlugin`
- System sets: `FovSystems::{Prepare, MarkDirty, Recompute, DebugDraw}`
- Runtime config and stats: `FovRuntimeConfig`, `FovStats`, `FovDebugSettings`, `FovDebugGizmos`
- Grid layer:
  `GridMapSpec`, `GridOpacityMap`, `GridFovConfig`, `GridFovBackend`, `GridCornerPolicy`, `GridFovResult`, `compute_grid_fov`
- Spatial layer:
  `SpatialVisibilityQuery`, `SpatialShape`, `SpatialDimension`, `VisibilityLayer`, `VisibilityLayerMask`, `VisibilityTestResult`
- ECS viewer components:
  `GridFov`, `GridFovState`, `SpatialFov`, `SpatialFovState`, `FovDirty`
- ECS world inputs:
  `FovTarget`, `FovOccluder`, `OccluderShape`
- Pure helpers:
  `evaluate_visibility`, `occluded_by_any`, `has_grid_line_of_sight`, `supercover_line`, `merge_grid_visibility`, `merge_spatial_visibility`

## Supported Algorithms

- Grid FOV:
  `GridFovBackend::RecursiveShadowcasting` is the default
- Grid LOS:
  supercover line stepping with configurable corner policy
- Spatial LOS:
  range-only or cone checks plus a final occlusion step

The API is intentionally open-ended. Consumers can keep the ECS integration and swap only the pure visibility logic, or keep the pure logic and provide their own world adapter.

## Supported World Representation Patterns

- `GridOpacityMap` for square-grid gameplay
- Pure callback-based grid opacity through `compute_grid_fov(...)` and LOS helpers
- ECS occluder primitives through `FovOccluder`
- Custom physics or nav data by calling `evaluate_visibility` with your own occlusion closure

## Behavior Notes

- `GridCornerPolicy::IgnoreAdjacentWalls` allows diagonal peeking through a one-cell pinch.
- `GridCornerPolicy::BlockIfBothAdjacentWalls` blocks only when both side-adjacent cells at a corner crossing are opaque.
- `GridCornerPolicy::BlockIfEitherAdjacentWall` is the strictest option and blocks on either side-adjacent wall.
- `GridFovConfig::reveal_blockers` controls whether opaque target cells count as visible.
- A cone with a zero-length forward vector degrades to range-only behavior instead of panicking.
- `SpatialFov::with_near_override(...)` lets very close targets bypass angular gating, which keeps perception stable near cone boundaries.
- Public state components are only rewritten when the published visibility sets actually change. That keeps `Changed<GridFovState>` and `Changed<SpatialFovState>` useful for downstream systems.

## Examples

| Example | Purpose | Run |
| --- | --- | --- |
| `basic_grid` | Minimal square-grid FOV with one moving viewer | `cargo run -p saddle-ai-fov --example basic_grid` |
| `exploration_memory` | Demonstrates `visible_now` plus persistent explored cells | `cargo run -p saddle-ai-fov --example exploration_memory` |
| `multi_viewers` | Merges multiple viewer states without coupling the viewers together | `cargo run -p saddle-ai-fov --example multi_viewers` |
| `cone_2d` | Top-down cone checks with simple occluder primitives and live debug rays | `cargo run -p saddle-ai-fov --example cone_2d` |
| `cone_3d` | 3D sentry vision with multi-sample targets and box occluders | `cargo run -p saddle-ai-fov --example cone_3d` |
| `saddle-ai-fov-lab` | Crate-local showcase with BRP and E2E hooks | `cargo run -p saddle-ai-fov-lab` |

## Crate-Local Lab

`shared/ai/saddle-ai-fov/examples/lab` is the verification app for this crate. It keeps the richer debug surface inside the shared crate instead of pushing it into the project-level sandboxes.

```bash
cargo run -p saddle-ai-fov-lab
```

E2E commands:

```bash
cargo run -p saddle-ai-fov-lab --features e2e -- smoke_launch
cargo run -p saddle-ai-fov-lab --features e2e -- fov_grid_memory
cargo run -p saddle-ai-fov-lab --features e2e -- fov_cone_occlusion
```

`fov_smoke` remains as a backward-compatible alias for `smoke_launch`.

## BRP

Useful BRP commands against the lab:

```bash
uv run --project .codex/skills/bevy-brp/script brp app launch saddle-ai-fov-lab
uv run --project .codex/skills/bevy-brp/script brp world query bevy_ecs::name::Name
uv run --project .codex/skills/bevy-brp/script brp world query saddle_ai_fov::components::GridFovState
uv run --project .codex/skills/bevy-brp/script brp world query saddle_ai_fov::components::SpatialFovState
uv run --project .codex/skills/bevy-brp/script brp resource get saddle_ai_fov::resources::FovStats
uv run --project .codex/skills/bevy-brp/script brp resource get saddle_ai_fov::debug::FovDebugSettings
uv run --project .codex/skills/bevy-brp/script brp extras screenshot /tmp/fov_lab.png
uv run --project .codex/skills/bevy-brp/script brp extras shutdown
```

## Limitations

- The built-in grid adapter is square-grid only. Hex support is not included in v1.
- `RecursiveShadowcasting` is the default backend, not a fully symmetric shadowcasting implementation.
- The ECS spatial occlusion layer uses simple discs, rectangles, spheres, and boxes. If you need collider-accurate 3D traces, call `evaluate_visibility` with your own adapter.
- The runtime tracks remembered cells and targets, but it does not publish a time-based forgetting policy in v1.

## More Docs

- [Architecture](docs/architecture.md)
- [Configuration](docs/configuration.md)
- [Algorithm Tradeoffs](docs/algorithm-tradeoffs.md)
