build:
    cargo build --release

run:
    cargo run --package sector --release

dev:
    cargo run --package sector --features bevy/dynamic

edit:
    cargo run --package sector_edit --release

dev-edit:
    cargo run --package sector_edit --features bevy/dynamic

serve-web: build-web
    miniserve --index index.html wasm

build-web:
    cargo build --release --target wasm32-unknown-unknown

    rm -rf wasm/target/
    wasm-bindgen --target web --no-typescript --out-dir wasm/target \
        target/wasm32-unknown-unknown/release/sector.wasm
    wasm-bindgen --target web --no-typescript --out-dir wasm/target \
        target/wasm32-unknown-unknown/release/sector_edit.wasm

    rm -rf wasm/assets/
    cp -R assets wasm/
