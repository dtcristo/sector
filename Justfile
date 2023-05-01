play:
    @just dev sector

edit:
    @just dev sector_edit

build BIN_NAME:
    cargo build --bin {{BIN_NAME}} --features {{BIN_NAME}}

release BIN_NAME:
    cargo build --bin {{BIN_NAME}} --features {{BIN_NAME}} --release

dev BIN_NAME:
    cargo run --bin {{BIN_NAME}} --features "{{BIN_NAME}} bevy/dynamic_linking"

run BIN_NAME:
    cargo run --bin {{BIN_NAME}} --features {{BIN_NAME}} --release

serve-web: build-web
    miniserve --index index.html wasm

build-web:
    cargo build --bin sector --features sector --release --target wasm32-unknown-unknown
    cargo build --bin sector_edit --features sector_edit --release --target wasm32-unknown-unknown

    rm -rf wasm/target/
    wasm-bindgen --target web --no-typescript --out-dir wasm/target \
        target/wasm32-unknown-unknown/release/sector.wasm
    wasm-bindgen --target web --no-typescript --out-dir wasm/target \
        target/wasm32-unknown-unknown/release/sector_edit.wasm

    rm -rf wasm/assets/
    cp -R assets wasm/
