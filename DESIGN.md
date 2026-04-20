# DESIGN

## Purpose

`sector` is a retro 2.5D sector/portal renderer built with Rust and Bevy. The project favors a software-rendered look over physical realism or modern rendering features: flat-shaded walls, banded distance falloff, crisp outlines, low-resolution presentation, and open-sky voids with flat sky tints or black fallback are intentional parts of the design.

The codebase is organized around a small runtime, a shared map/world representation, and fast headless tests that protect rendering and movement behavior.

## Design goals

- Keep rendering and simulation simple enough to reason about and test.
- Preserve the retro aesthetic with obvious shade bands and sharp one-pixel boundaries.
- Treat performance as a primary concern in runtime and data format decisions.
- Prefer explicit data and validation over permissive map loading.
- Keep the runtime and editor loosely coupled through a shared map format.

## Top-level architecture

The repository currently has four primary binaries:

| Binary | Entry point | Purpose |
| --- | --- | --- |
| `sector` | `src/bin/sector/main.rs` | Runtime/player application using the software renderer |
| `sector_edit` | `src/bin/sector_edit/main.rs` | egui-based map editor for the shared sector data |
| `sector_import_doom` | `src/bin/sector_import_doom/main.rs` | CLI importer that converts DOOM WAD maps into validated sector-map assets |
| `sector_validate` | `src/bin/sector_validate/main.rs` | CLI validation pass for map assets |

The shared library code lives in `src/`:

- `src/map.rs`: RON map asset format, load/save helpers, wasm embedded-map lookup, and structural validation.
- `src/world.rs`: runtime sector types and wall expansion helpers.
- `src/game/`: player state, input, and physics/movement simulation.
- `src/render/`: software renderer, automap, projection math, and frame utilities.
- `src/bin/sector_import_doom/importer.rs`: WAD parsing, color extraction, geometry conversion, and map emission for imported DOOM levels.
- `src/color.rs`, `src/geometry.rs`, `src/player.rs`: shared primitive types and gameplay constants.
- `build.rs`: scans shipped maps and generates the embedded map registry used by the wasm runtime.

## Data model

### Runtime world

The runtime world is intentionally small:

- `Sector` stores an id, clockwise convex polygon vertices, per-wall colors, optional per-wall portal targets, per-portal walkability flags, optional portal trim colors, flat `floor`/`ceil` heights, per-sector `floor_color`/`ceil_color`, and a `no_ceiling` render flag.
- `WallSegment` is derived from `Sector` and expands the implicit polygon loop into explicit wall edges.
- `InitialSector` marks the sector containing the spawn.

This is a classic sector graph rather than a general polygon soup. Each sector is a flat-prism volume with a single floor height and single ceiling height.

### Asset format

Maps are stored as either RON (`*.map.ron`) or MessagePack (`*.map.mp`) in `assets/maps/`. `SectorMap` mirrors the runtime world but stays asset-friendly:

- `initial_sector`
- `initial_position`
- `initial_direction_degrees`
- `sectors`
  - `floor`
  - `ceil`
  - optional `floor_color`
  - optional `ceil_color`
  - optional `no_ceiling`
  - `vertices`
  - `walls`
    - `color`
    - optional `portal`
    - optional `walkable` (defaults to `true`)
    - optional `upper_color`
    - optional `lower_color`

Flat wall, floor, and ceiling colors are the material system today. There are no textures, no slopes, and no per-surface UVs. A sector without a rendered ceiling still keeps a numeric ceiling height for collision and portal opening checks, and may optionally carry a `sky_color` so open ceilings can render as a flat sky tint instead of the black fallback.

## Runtime flow

### Startup

The runtime:

1. Resolves a map source.
2. Validates it through `src/map.rs`.
3. Converts `SectorMap` into runtime `Sector` components.
4. Spawns one `Player` entity and one entity per sector.
5. Uses the map's explicit spawn position and facing direction.

On native builds, the runtime accepts an optional map path argument and otherwise falls back to `DEFAULT_MAP_FILE_PATH` from `src/lib.rs`.

On wasm builds, the runtime derives the map name from the browser location (`/`, `/default`, `/e1m1`, or `#e1m1`) and loads the map from a build-generated embedded registry. This avoids filesystem access in the browser while keeping route-based map switching dynamic across whatever maps were present in `assets/maps/` when the web bundle was built.

### Player and movement

The player simulation is built around a small first-person controller:

- fixed physical dimensions from `src/player.rs`
- walk/strafe movement in the horizontal plane
- jump using earth gravity
- grounded crouch that lowers eye height, plus airborne crouch that lifts the feet instead
- step-up support using `PLAYER_MAX_STEP_HEIGHT_METERS`
- sector resolution that prefers the current or adjacent portal sector when possible

Horizontal movement is resolved against sector walls and portal openings. A portal behaves like passable space only when both sides mark it walkable and the destination sector offers enough vertical clearance and acceptable step height. Otherwise it behaves like a solid wall even though the renderer may still draw through it.

Airborne crouching is intentionally slightly gamey: the camera stays fixed while the collision capsule shortens upward, which allows limited crouch-jump behavior for ledges that are just out of reach with a normal jump.

The runtime also exposes a lightweight console debug path: pressing `?` prints a RON-style snapshot of the player's movement state plus the current sector's geometry and portal connections so map and physics issues can be inspected without adding a heavyweight debug UI.

### Rendering

The renderer is a software rasterizer over a fixed 320x240 buffer scaled to the window. The visual style is deliberately limited:

- flat wall colors instead of textures
- horizontal floor/ceiling bands colored per sector
- optional open-sky sectors that either use a flat `sky_color` tint or fall back to black when the ceiling is skipped
- quantized distance shading (`SHADE_BANDS = 16`)
- explicit black outlines between materially or geometrically distinct surfaces

At a high level:

1. Determine root sectors from the current view position.
2. Traverse visible sectors through a portal queue.
3. Clip walls against the near plane and horizontal frustum.
4. Project wall columns and flat floor/ceiling spans, skipping ceiling spans for `no_ceiling` sectors.
5. Shade by distance using a banded brightness curve.
6. Apply a post-pass outline mask so seams stay crisp and single-pixel thick.

The renderer is portal-based, not BSP-based. It depends on valid reciprocal portal topology and convex sectors to stay simple.

When two adjacent portal-connected sectors both use `no_ceiling`, the renderer suppresses the upper trim between them so imported sky openings read as one continuous sky span instead of a floating wall band.

### Automap

The automap shares the world data and projection helpers with the renderer. It supports:

- rotating full map
- rotating visible-only map
- north-up full map
- north-up visible-only map

Portal edges are deduplicated so a shared portal is only drawn once.

## Map validation rules

`validate_map` is intentionally strict. Current invariants include:

- initial sector index must exist
- spawn must be inside the initial sector
- spawn must have enough wall clearance for the player radius
- initial sector must have enough headroom for the player
- each sector must have at least three vertices
- floor must be below ceiling
- wall count must match vertex count
- walls must have non-zero length
- sectors must wind clockwise
- sectors must be convex
- portal targets must exist
- portals must be reciprocal across the reversed edge
- reciprocal portals must agree on walkability
- portals must have overlapping vertical openings
- non-portal shared edges cannot create zero-thickness solid walls
- sectors cannot overlap in plan view while also overlapping vertically

The validator is a core design tool, not just a safety net. The renderer and movement code assume these invariants rather than defending against arbitrary malformed geometry at runtime.

## DOOM import pipeline

`sector_import_doom` is a content pipeline layered on top of the normal map format rather than a runtime feature path. It imports a WAD map, converts it to `SectorMap`, and saves through the same validation and serialization helpers as hand-authored maps.

The importer and its geometry dependencies are intentionally gated behind the `doom_import` feature so the web/runtime build does not drag the Doom conversion stack or its native-only dependency graph into wasm.

The importer currently:

1. Parses the WAD directory and the selected map lumps directly.
2. Reads the palette, patch tables, textures, and flats so it can average source art into flat wall/floor/ceiling colors.
3. Builds plan-view polygons from Doom linedefs, then decomposes those shapes into convex cells acceptable to this engine.
4. Scales XY and Z from Doom units so the current player radius and eye height line up with Doom's feel.
5. Emits view-only portals for impassable linedefs, converts sky ceilings into `no_ceiling` sectors with a flat tint derived from the map's sky texture, and opens door sectors by following Doom door linedef specials and lifting those sectors to neighboring ceiling heights.
6. Saves the generated map through the normal validation pipeline, typically as MessagePack for imported content.

This keeps imported maps honest: if the converted result cannot satisfy the same convexity, portal, overlap, spawn, and clearance rules as native content, the import fails instead of shipping a broken asset.

## Editor design

The editor is intentionally secondary to the runtime. It is an egui-based direct manipulator over the shared sector data:

- loads the same RON asset format
- exposes per-sector heights and wall/vertex data
- saves through the same map conversion and validation pipeline

It currently operates on the default map path and should be treated as a lightweight tooling surface rather than the center of the architecture.

## Testing strategy

The project leans on fast unit tests instead of heavy end-to-end harnesses:

- map tests verify validation rules and asset expectations
- physics tests cover collision, stepping, jumping, crouching, and portal transitions
- renderer tests cover filling behavior, portal continuity, shading, and outline behavior
- automap tests cover visible/full modes and portal edge handling

This keeps feedback quick while still protecting the important visual and gameplay invariants.

## Current constraints and simplifications

These are intentional current limits of the system:

- sectors must be convex
- floors and ceilings are flat per sector
- walls, floors, and ceilings use flat colors rather than textures
- no native concept of doors, lifts, or moving geometry
- no textured skybox support yet; `no_ceiling` sectors currently reveal either a flat `sky_color` tint or the black fallback
- no stacked sectors occupying the same 2D footprint with overlapping height ranges
- editor support exists, but runtime/rendering quality and performance take priority

Those constraints are why ports of more complex source material, like DOOM maps, need convex decomposition and feature adaptation instead of a one-to-one feature translation.

## Operational workflow

The working conventions tied to the current design are:

- format with `cargo fmt --all`
- check with `cargo check --all --features "sector sector_edit doom_import"`
- run the existing tests with `cargo test --features "sector sector_edit doom_import"`
- validate map assets after map changes
- CI/CD should mirror that native verification set before building and deploying the wasm runtime bundle

`README.md` should describe the user-facing workflow, while this file should remain the durable source of truth for architecture and design intent.
