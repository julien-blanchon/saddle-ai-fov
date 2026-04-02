# Algorithm Tradeoffs

## Why `RecursiveShadowcasting` Is The Default

The crate defaults to `GridFovBackend::RecursiveShadowcasting` because it is the best first-pass balance for this shared runtime:

- fast enough for many small or medium-radius viewers
- time-tested in roguelikes and tactics games
- easy to keep fully CPU-side and unit-testable
- compatible with `reveal_blockers`
- easy to pair with explicit LOS refinement for corner policy behavior

This backend does **not** claim perfect symmetry. The crate keeps that explicit so consumers do not make the wrong assumptions.

## Why There Is Also `RaycastLos`

`GridFovBackend::RaycastLos` exists for two reasons:

1. It gives consumers a simpler mental model for debugging visibility edge cases.
2. It provides a useful comparison backend when validating maps, corner policy choices, or future alternate backends.

The tradeoff is cost: it scales more directly with the number of candidate cells and the length of each line trace.

## Corner Policy Is Separate From Backend

`GridCornerPolicy` is deliberately its own axis instead of being hidden inside the backend choice.

That keeps gameplay-facing choices explicit:

- allow diagonal peeking
- block only when both adjacent side cells are walls
- block on either adjacent wall

Those choices matter just as much as the broad algorithm family.

## Why The Spatial Layer Is Callback-Based

The crate does not hard-depend on Avian, Rapier, or a project-specific collision world.

Instead:

- `SpatialVisibilityQuery` describes the viewer
- `evaluate_visibility` handles range and cone logic
- the caller supplies occlusion

That keeps the crate reusable in:

- pure ECS sandboxes
- turn-based tactics games
- projects with custom tile or portal visibility
- projects that want to plug a physics raycast in later

The ECS integration ships a default geometry adapter so the crate is still useful out of the box.

## Remaining Artifacts In V1

- `RecursiveShadowcasting` plus LOS refinement is still a practical compromise, not a mathematically exact visibility solution.
- The built-in ECS occluder layer uses simple primitive intersection, not physics-accurate mesh traces.
- The runtime tracks remembered cells and targets, but it does not ship time-based forgetting or hysteresis in v1.
- The square-grid adapter is intentionally square-grid only. Hex support is a future extension, not an implied guarantee.

## When To Prefer A Different Strategy

- Prefer `RaycastLos` when validating or debugging exact doorway and corner behavior on small maps.
- Prefer a custom occlusion closure when you already have a robust physics or nav visibility query in your game.
- Prefer a future symmetric or permissive backend if your game design depends heavily on symmetry or peeking guarantees and the default recursive behavior is not a good fit.
