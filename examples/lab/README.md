# `fov_lab`

Crate-local showcase and verification app for `fov`.

It keeps the richer runtime surface inside the shared crate:

- left side: grid FOV with exploration memory
- right side: 2D cone visibility with occluder primitives
- BRP enabled in `dev`
- E2E scenarios behind the `e2e` feature

## Run

```bash
cargo run -p fov_lab
```

## E2E

```bash
cargo run -p fov_lab --features e2e -- smoke_launch
cargo run -p fov_lab --features e2e -- fov_grid_memory
cargo run -p fov_lab --features e2e -- fov_cone_occlusion
```

`fov_smoke` remains as a backward-compatible alias for `smoke_launch`.

## BRP

```bash
uv run --project .codex/skills/bevy-brp/script brp app launch fov_lab
uv run --project .codex/skills/bevy-brp/script brp world query bevy_ecs::name::Name
uv run --project .codex/skills/bevy-brp/script brp world query fov::components::GridFovState
uv run --project .codex/skills/bevy-brp/script brp world query fov::components::SpatialFovState
uv run --project .codex/skills/bevy-brp/script brp extras screenshot /tmp/fov_lab.png
uv run --project .codex/skills/bevy-brp/script brp extras shutdown
```
