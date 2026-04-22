# AGENTS.md

- Project: Bevy 2.5D sector/portal renderer. The egui map editor exists, but it is currently low priority and should not receive attention unless explicitly requested.

## Priorities

- Maintain a constant focus on performance. Treat algorithmic and rendering performance as critical in every change.
- Keep the retro aesthetic in runtime and tooling changes: obvious shading bands, crisp one-pixel seams, and no smoothing that softens the software-rendered look.
- Keep the architecture clean. Break code into logical files/modules when it improves clarity and maintenance.
- Keep tests fast, prefer small headless tests, and make sure all meaningful changes have appropriate test coverage.

## Repository landmarks

- Runtime entry: `src/bin/sector/main.rs`
- Editor entry: `src/bin/sector_edit/main.rs`
- Core library: `src/game/` (player + physics), `src/render/` (software renderer), `src/map.rs` (RON/Protobuf map formats), `src/world.rs` (shared sector data)
- Maps live in `assets/maps/*.map.ron` and `assets/maps/*.map.pb`
- Map dimensions are meters, spawn lives in the map, and sector winding should stay clockwise for stable rendering
- Main commands:
  - `cargo test --features "sector sector_edit"`
  - `cargo run --features sector --bin sector`
  - `cargo run --features sector_edit --bin sector_edit`
  - `cargo run --bin sector_validate -- assets/maps/default.map.ron`

## Working rules

- Always format code with `cargo fmt --all` after changes.
- Always run `cargo check --all` after changes and fix issues.
- Validate maps after making any map-related changes.
- Write automated tests where appropriate.
- Never push commits. Leave pushing to the user.
- Commit logical changes in discrete commits.
- Each commit message should explain what changed and why, not just use a short one-line summary.
- Update `README.md` whenever the user-facing workflow or project documentation should change.
- Keep `DESIGN.md` updated as the architecture/design evolves.
- Keep `TODO.md` up to date with future work and follow-up tasks.
- When a prompt contains multiple task paragraphs separated by blank lines, execute them systematically in prompt order, one task/paragraph at a time, and commit after each task is complete.
- Use internal todo tracking so no prompted task gets lost.
- Compact the thread when necessary, including between major tasks when context usage is getting large.

## AGENTS.md maintenance

- Store durable agent memories and project-wide operating instructions here.
- If the user asks for different standing behavior, persist that instruction here.
- Keep this file updated as project assumptions, workflows, priorities, or durable instructions change.
