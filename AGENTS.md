# AGENTS.md

- Bevy 0.18 Rust project for a 2.5D sector/portal renderer plus a small egui map editor.
- Runtime entry: `src/bin/sector/main.rs`. Editor entry: `src/bin/sector_edit/main.rs`.
- Core library: `src/game/` (player + physics), `src/render/` (software renderer), `src/map.rs` (RON map format), `src/world.rs` (shared sector data).
- Maps live in `assets/maps/*.map.ron`; dimensions are meters, spawn lives in the map, and sector winding should stay clockwise for stable rendering.
- Main commands: `cargo test --features "sector sector_edit"`, `cargo run --features sector --bin sector`, `cargo run --features sector_edit --bin sector_edit`.
- Prefer small headless tests, keep map data human-readable, and do not store ECS entity IDs in map files.
