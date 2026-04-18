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

`sector` is an experimental software-rendered engine for Doom-style 2.5D environments. It uses convex sectors, explicit portals, flat floor/ceiling planes, and per-wall colors to produce a crisp retro look with banded shading and single-pixel seams.

The runtime and editor share the same RON map format in `assets/maps/*.map.ron`.

## Repository docs

- [`DESIGN.md`](DESIGN.md): current architecture, renderer/runtime design, map format, and system constraints
- [`TODO.md`](TODO.md): future improvements and follow-up work

## Repository layout

- `src/bin/sector/main.rs`: runtime entry point
- `src/bin/sector_edit/main.rs`: egui-based editor entry point
- `src/bin/sector_validate/main.rs`: standalone map validator
- `src/game/`: player input and physics
- `src/render/`: software renderer, automap, and projection math
- `src/map.rs`: map asset loading, saving, and validation
- `src/world.rs`: runtime sector and wall data
- `assets/maps/`: shipped map assets

## Common commands

```sh
cargo test --features "sector sector_edit"
cargo run --features sector --bin sector
cargo run --features sector_edit --bin sector_edit
cargo run --bin sector_validate -- assets/maps/default.map.ron
```

If you use `just`, the current shortcuts are:

```sh
just play
just edit
just validate-map
```

## Map authoring notes

- Map coordinates are in meters.
- Spawn position and facing direction live in the map asset.
- Sectors must wind clockwise and remain convex.
- Portals must be reciprocal and provide real vertical openings.
- Wall colors are the current material system; there is no texture support yet.

Run map validation after map changes to catch winding, overlap, portal, and spawn issues early.

## Current scope

The project currently focuses on:

- fast headless validation and rendering tests
- portal-based software rendering
- simple first-person movement with stepping, jumping, and crouching
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
