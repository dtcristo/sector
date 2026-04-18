# DESIGN

## Purpose

`sector` is a retro 2.5D sector/portal renderer built with Rust and Bevy. The project favors a software-rendered look over physical realism or modern rendering features: flat-shaded walls, banded distance falloff, crisp outlines, low-resolution presentation, and black-sky voids are intentional parts of the design.

The codebase is organized around a small runtime, a shared map/world representation, and fast headless tests that protect rendering and movement behavior.

## Design goals

- Keep rendering and simulation simple enough to reason about and test.
- Preserve the retro aesthetic with obvious shade bands and sharp one-pixel boundaries.
- Treat performance as a primary concern in runtime and data format decisions.
- Prefer explicit data and validation over permissive map loading.
- Keep the runtime and editor loosely coupled through a shared map format.

## Top-level architecture

The repository currently has three primary binaries:

| Binary | Entry point | Purpose |
| --- | --- | --- |
| `sector` | `src/bin/sector/main.rs` | Runtime/player application using the software renderer |
| `sector_edit` | `src/bin/sector_edit/main.rs` | egui-based map editor for the shared sector data |
| `sector_validate` | `src/bin/sector_validate/main.rs` | CLI validation pass for map assets |

The shared library code lives in `src/`:

- `src/map.rs`: RON map asset format, load/save helpers, and structural validation.
- `src/world.rs`: runtime sector types and wall expansion helpers.
- `src/game/`: player state, input, and physics/movement simulation.
- `src/render/`: software renderer, automap, projection math, and frame utilities.
- `src/color.rs`, `src/geometry.rs`, `src/player.rs`: shared primitive types and gameplay constants.

## Data model

### Runtime world

The runtime world is intentionally small:

- `Sector` stores an id, clockwise convex polygon vertices, per-wall colors, optional per-wall portal targets, per-portal walkability flags, optional portal trim colors, flat `floor`/`ceil` heights, per-sector `floor_color`/`ceil_color`, and a `no_ceiling` render flag.
- `WallSegment` is derived from `Sector` and expands the implicit polygon loop into explicit wall edges.
- `InitialSector` marks the sector containing the spawn.

This is a classic sector graph rather than a general polygon soup. Each sector is a flat-prism volume with a single floor height and single ceiling height.

### Asset format

Maps are stored as RON in `assets/maps/*.map.ron`. `SectorMap` mirrors the runtime world but stays asset-friendly:

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

Flat wall, floor, and ceiling colors are the material system today. There are no textures, no slopes, and no per-surface UVs. A sector without a rendered ceiling still keeps a numeric ceiling height for collision, portal opening checks, and future sky rendering.

## Runtime flow

### Startup

The runtime:

1. Loads a map asset from disk.
2. Validates it through `src/map.rs`.
3. Converts `SectorMap` into runtime `Sector` components.
4. Spawns one `Player` entity and one entity per sector.
5. Uses the map's explicit spawn position and facing direction.

The runtime accepts an optional map path argument and otherwise falls back to `DEFAULT_MAP_FILE_PATH` from `src/lib.rs`.

### Player and movement

The player simulation is built around a small first-person controller:

- fixed physical dimensions from `src/player.rs`
- walk/strafe movement in the horizontal plane
- jump using earth gravity
- crouch with reduced height and eye height
- step-up support using `PLAYER_MAX_STEP_HEIGHT_METERS`
- sector resolution that prefers the current or adjacent portal sector when possible

Horizontal movement is resolved against sector walls and portal openings. A portal behaves like passable space only when both sides mark it walkable and the destination sector offers enough vertical clearance and acceptable step height. Otherwise it behaves like a solid wall even though the renderer may still draw through it.

### Rendering

The renderer is a software rasterizer over a fixed 320x240 buffer scaled to the window. The visual style is deliberately limited:

- flat wall colors instead of textures
- horizontal floor/ceiling bands colored per sector
- optional black-sky sectors that skip ceiling rasterization entirely
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
- no textured skybox support yet; `no_ceiling` sectors currently reveal black sky
- no stacked sectors occupying the same 2D footprint with overlapping height ranges
- editor support exists, but runtime/rendering quality and performance take priority

Those constraints are why ports of more complex source material, like DOOM maps, need adaptation instead of a one-to-one feature translation.

## Operational workflow

The working conventions tied to the current design are:

- format with `cargo fmt --all`
- check with `cargo check --all`
- run the existing tests with `cargo test --features "sector sector_edit"`
- validate map assets after map changes

`README.md` should describe the user-facing workflow, while this file should remain the durable source of truth for architecture and design intent.
