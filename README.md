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

`sector` is an experimental software-rendered engine for Doom-style 2.5D environments. It uses convex sectors, explicit portals, flat floor/ceiling planes, optional open ceilings with either black fallback or flat sky tint, and per-surface flat colors to produce a crisp retro look with banded shading and single-pixel seams.

The native runtime and editor share the same `SectorMap` data model across both RON and Protobuf assets in `assets/maps/*.map.ron` and `assets/maps/*.map.pb`. The runtime now treats 4:3 as the baseline view but adapts its logical render buffer to the current window, widening out to 21:9 or growing vertically to 9:16 before letterboxing extreme shapes. The web build ships the play runtime only, bundles the shipped maps, and resolves the current map from the URL path.

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
cargo test --features "sector sector_edit doom_import"
cargo run --features sector --bin sector -- assets/maps/default.map.ron
cargo run --features sector --bin sector -- assets/maps/e1m1.map.pb
cargo run --features sector_edit --bin sector_edit
cargo run --bin sector_import_doom --features doom_import -- ../DOOM1.WAD E1M1
cargo run --bin sector_validate -- assets/maps/default.map.ron
```

If you use `just`, the current shortcuts are:

```sh
just play
just play default
just play e1m1
just edit
just build-web
just serve-web
just import-doom ../DOOM1.WAD E1M1
just validate
just validate default
just validate e1m1
```

`just play` and `just validate` both default to the `default` map when no map name is provided.

While playing, press `Shift+/` (`?`) to print a RON-style runtime state dump to the console with the player's position, velocity, facing, resolved sector, movement flags, and current sector wall/portal data.
Left click captures the cursor for play, right click or `Escape` releases it, `N` toggles noclip, and a double tap on `Space` toggles fly mode. While flying, hold `Space` to rise and `Ctrl` to descend; clipping still applies unless noclip is also enabled. Movement simulation now runs on a fixed Bevy timestep for steadier behavior across frame rates. As the window changes size, the game resizes its live pixel buffer and FOV together instead of staying locked to a single 320x240 view; 4:3 remains the preferred baseline, while wider windows reveal more horizontally and taller windows reveal more vertically.

## Editor

`sector_edit` is now a native-only egui map authoring tool over the shared map format. It can:

- create a new starter map, open/reload existing maps, and save or save-as both `.map.ron` and `.map.pb` files
- validate the current document on save before it writes anything
- edit sector heights, wall colors, trims, portal walkability, spawn position, and spawn facing
- center the map view around the spawn on load and give you explicit pan/zoom controls while editing
- draft new rooms directly in the map view, splitting concave outlines into convex sectors automatically
- rebuild portals by matching reversed wall edges across adjacent sectors
- launch the runtime beside the editor so you can save and replay the current map quickly

## Web build

`just build-web` builds the browser version of the play runtime only. The editor and Doom importer are native-only tools.

`just serve-web` serves the `wasm/` directory as a small SPA so map routes work locally. The browser runtime resolves the map from the URL path:

- `/` or `/default` loads the default map
- `/e1m1` loads E1M1

Map names are not hardcoded in the runtime; the web bundle scans `assets/maps/` at build time and embeds every shipped map so new maps can be exposed by route after rebuilding the web output.

The browser build uses the same adaptive viewport rules as native play, so route-selected maps keep the same 4:3 baseline feel while still making better use of wide and tall windows. The web runtime now goes back through `bevy_pixels` itself instead of the temporary 2D canvas fallback, using the sibling `../bevy_pixels` checkout while those wasm fixes are still local-only.

## CI/CD

`.github/workflows/ci-cd.yml` runs formatting, native checks, tests, shipped-map validation, and the web bundle build on pushes and pull requests. Pushes to `main` then deploy the generated `wasm/` bundle to Cloudflare Pages project `sector`.

## Shipped maps

- `default`: hand-authored testbed map for movement, rendering, stairs, portals, crouch spaces, and overlapping-height rooms
- `e1m1`: imported from the DOOM shareware WAD as Protobuf, with door sectors held open from Doom door specials, zero-height door sectors, and door-texture heuristics while keeping the actual doorways walkable, sky sectors converted into `no_ceiling` spaces tinted from the map sky texture, spawn/facing matched to the Doom start, and wall/floor/ceiling colors derived from the average colors of the source textures and flats

## DOOM import workflow

`sector_import_doom` imports a WAD map by lump name and writes `assets/maps/<map-id-lowercase>.map.pb` by default:

```sh
cargo run --bin sector_import_doom --features doom_import -- ../DOOM1.WAD E1M1
just import-doom ../DOOM1.WAD E1M1
```

The importer:

- preserves the Doom player start position and facing direction relative to the current player model
- decomposes Doom sectors into convex cells that satisfy this engine's validation rules
- converts `F_SKY1` ceilings into `no_ceiling` sectors, tinting them from the correct Doom sky texture for the imported map
- opens Doom door sectors from linedef specials, zero-height sectors, and door-texture hints, then clears those doorway portals so imported doors stay traversable
- keeps non-door impassable linedefs as view-only portals
- averages wall textures and flats into this engine's flat-color material model

Pass a third argument to the binary if you want a different output path.

## Map authoring notes

- Map coordinates are in meters.
- Spawn position and facing direction live in the map asset.
- Floor and ceiling planes can carry their own flat colors through `floor_color` and `ceil_color`.
- Sectors must wind clockwise and remain convex.
- Set `no_ceiling: true` on a sector to leave its ceiling open while still keeping its collision ceiling height. Leave `sky_color` unset for black sky, or set it to a flat tint for imported/open-sky sectors.
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

The editor now supports real file workflows and map drafting, but runtime/rendering quality and performance are still the main priorities.

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
