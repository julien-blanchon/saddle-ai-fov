# `saddle-ai-fov-lab`

Crate-local showcase and verification app for `saddle-ai-fov`.

It keeps the richer runtime surface inside the shared crate:

- left side: grid FOV with exploration memory
- right side: spatial visibility, neutral stimulus, and the optional stealth mapper
- BRP enabled in `dev`
- E2E scenarios behind the `e2e` feature

## Run

```bash
cd examples
cargo run -p saddle-ai-fov-lab
```

## E2E

```bash
cd examples
cargo run -p saddle-ai-fov-lab --features e2e -- smoke_launch
cargo run -p saddle-ai-fov-lab --features e2e -- fov_grid_memory
cargo run -p saddle-ai-fov-lab --features e2e -- fov_cone_occlusion
cargo run -p saddle-ai-fov-lab --features e2e -- fov_guard_range_cutoff
cargo run -p saddle-ai-fov-lab --features e2e -- fov_stimulus_pipeline
```

`fov_smoke` remains as a backward-compatible alias for `smoke_launch`, and `fov_awareness_detection` remains as a backward-compatible alias for `fov_stimulus_pipeline`.

## BRP

```bash
uv run --project .codex/skills/bevy-brp/script brp app launch saddle-ai-fov-lab
uv run --project .codex/skills/bevy-brp/script brp world query bevy_ecs::name::Name
uv run --project .codex/skills/bevy-brp/script brp world query saddle_ai_fov::components::GridFovState
uv run --project .codex/skills/bevy-brp/script brp world query saddle_ai_fov::components::SpatialFovState
uv run --project .codex/skills/bevy-brp/script brp world query saddle_ai_fov::StealthAwarenessState
uv run --project .codex/skills/bevy-brp/script brp extras screenshot /tmp/fov_lab.png
uv run --project .codex/skills/bevy-brp/script brp extras shutdown
```
