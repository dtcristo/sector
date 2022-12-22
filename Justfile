run:
    cargo run --release

dev:
    cargo run --features bevy/dynamic

build:
    cargo build --release

edit:
    cargo run --release --package sector_edit

dev-edit:
    cargo run --package sector_edit --features bevy/dynamic

build-edit:
    cargo build --release --package sector_edit

serve-web: build-web
    miniserve --index index.html wasm

build-web:
    rm -rf wasm/target/
    cargo build --release --target wasm32-unknown-unknown
    wasm-bindgen --target web --no-typescript --out-dir wasm/target \
        target/wasm32-unknown-unknown/release/sector.wasm
