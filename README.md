<div align="center">
  <h1>
    sector
  </h1>
  <p>
    <strong>
      Retro 2.5D sector/portal renderer built with Rust and Bevy
    </strong>
  </p>
</div>

## Overview

`sector` is an experimental software-rendered engine for Doom-style 2.5D environments. It uses convex sectors, explicit portals, flat floor/ceiling planes, optional black-sky ceilings, and per-surface flat colors to produce a crisp retro look with banded shading and single-pixel seams.

The runtime and editor share the same RON map format in `assets/maps/*.map.ron`.

## Repository docs

- [`DESIGN.md`](DESIGN.md): current architecture, renderer/runtime design, map format, and system constraints
- [`TODO.md`](TODO.md): future improvements and follow-up work

## Repository layout

- `src/bin/sector/main.rs`: runtime entry point
- `src/bin/sector_edit/main.rs`: egui-based editor entry point
- `src/bin/sector_import_doom/main.rs`: DOOM WAD to sector-map importer
- `src/bin/sector_validate/main.rs`: standalone map validator
- `src/game/`: player input and physics
- `src/render/`: software renderer, automap, and projection math
- `src/map.rs`: map asset loading, saving, and validation
- `src/world.rs`: runtime sector and wall data
- `assets/maps/`: shipped map assets

## Common commands

```sh
cargo test --features "sector sector_edit"
cargo run --features sector --bin sector -- assets/maps/default.map.ron
cargo run --features sector --bin sector -- assets/maps/e1m1.map.ron
cargo run --features sector_edit --bin sector_edit
cargo run --bin sector_import_doom -- ../DOOM1.WAD E1M1
cargo run --bin sector_validate -- assets/maps/default.map.ron
```

If you use `just`, the current shortcuts are:

```sh
just play
just play default
just play e1m1
just edit
just import-doom ../DOOM1.WAD E1M1
just validate
just validate default
just validate e1m1
```

`just play` and `just validate` both default to the `default` map when no map name is provided.

While playing, press `Shift+/` (`?`) to print a RON-style runtime state dump to the console with the player's position, velocity, facing, resolved sector, and current sector wall/portal data.

## Shipped maps

- `default`: hand-authored testbed map for movement, rendering, stairs, portals, crouch spaces, and overlapping-height rooms
- `e1m1`: imported from the DOOM shareware WAD, with doors held open, sky sectors converted into black-sky `no_ceiling` spaces, spawn/facing matched to the Doom start, and wall/floor/ceiling colors derived from the average colors of the source textures and flats

## DOOM import workflow

`sector_import_doom` imports a WAD map by lump name and writes `assets/maps/<map-id-lowercase>.map.ron` by default:

```sh
cargo run --bin sector_import_doom -- ../DOOM1.WAD E1M1
just import-doom ../DOOM1.WAD E1M1
```

The importer:

- preserves the Doom player start position and facing direction relative to the current player model
- decomposes Doom sectors into convex cells that satisfy this engine's validation rules
- converts `F_SKY1` ceilings into `no_ceiling` sectors and keeps impassable linedefs as view-only portals
- averages wall textures and flats into this engine's flat-color material model

Pass a third argument to the binary if you want a different output path.

## Map authoring notes

- Map coordinates are in meters.
- Spawn position and facing direction live in the map asset.
- Floor and ceiling planes can carry their own flat colors through `floor_color` and `ceil_color`.
- Sectors must wind clockwise and remain convex.
- Set `no_ceiling: true` on a sector to leave its ceiling unrendered as black sky while still keeping its collision ceiling height.
- Portal walls can be marked `walkable: false` to create windows or skybox openings that render through to another sector but block traversal.
- Portals must be reciprocal, agree on walkability, and provide real vertical openings.
- Flat wall, floor, and ceiling colors are the current material system; there is no texture support yet.

Run map validation after map changes to catch winding, overlap, portal, and spawn issues early.

## Current scope

The project currently focuses on:

- fast headless validation and rendering tests
- portal-based software rendering
- simple first-person movement with stepping, jumping, crouching, and modest crouch-jumps
- low-level map experimentation

The editor exists, but runtime/rendering quality and performance are the main priorities.

## Credits

This project would not be possible without educational material and inspiration from:

- [Bisqwit](https://www.youtube.com/c/Bisqwit) and [Portal Rendering Example Program](https://bisqwit.iki.fi/jutut/kuvat/programming_examples/portalrendering.html)
- [3DSage](https://www.youtube.com/c/3DSage) and [Let's Program Doom](https://www.youtube.com/watch?v=huMO4VQEwPc)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you shall be dual licensed as above, without any additional terms or conditions.
