# TODO

## Runtime and gameplay

- Add interactive doors, lifts, and other moving sector boundaries instead of baking them open into maps.
- Support richer map startup options, including selecting maps and spawn points from the runtime and editor UI.
- Add simple gameplay objects such as pickups, switches, keys, and scripted triggers.
- Improve player interaction feedback for low ceilings, blocked portals, and invalid transitions.
- Add optional noclip/debug movement for map inspection and regression triage.

## Renderer

- Introduce textured walls, floors, and ceilings without losing the crisp software-rendered look.
- Add sky or outdoor rendering semantics instead of approximating exterior space with very tall ceilings.
- Continue profiling portal traversal and column drawing to reduce overdraw and unnecessary work.
- Explore wider support for stacked spaces or room-over-room approximations that preserve current performance goals.
- Add more renderer regression coverage for large imported maps and unusual portal topologies.

## Map format and tooling

- Build reusable import/export tooling for external formats such as DOOM WAD data.
- Add helpers that decompose complex shapes into valid convex sectors automatically.
- Improve map validation diagnostics with clearer geometry context and suggested fixes.
- Let the editor open and save arbitrary map files instead of centering everything on the default map.
- Add map metadata for themes, authoring notes, and per-map tuning values.

## Editor

- Raise the editor above its current prototype state with better selection, snapping, and portal authoring workflows.
- Add visual feedback for invalid winding, non-convex sectors, missing reciprocal portals, and overlap problems while editing.
- Show spawn position and facing direction directly in the map view and allow editing them interactively.
- Add color-picking and palette workflows that fit the project's retro look.
- Add undo/redo and safer save flows.

## Testing and quality

- Add regression tests that cover additional shipped maps beyond `default`.
- Add focused tests for imported-map edge cases such as long corridors, tight door clearances, and large outdoor approximations.
- Add performance benchmarks for the renderer and movement simulation on representative maps.
- Add screenshot or frame-diff tooling for stable visual regression checks where it stays fast enough.

## Documentation

- Keep `DESIGN.md` aligned with new engine capabilities and data-model changes.
- Expand `README.md` examples as the map workflow and runtime options grow.
- Document map-authoring conventions more explicitly, including portal trims, spawn placement, and color selection guidance.
