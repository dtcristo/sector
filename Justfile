run:
    cargo run --release

build:
    cargo build --release

serve-web: build-web
    miniserve --index index.html wasm

build-web:
    rm -rf wasm/target/
    cargo build --release --target wasm32-unknown-unknown
    wasm-bindgen --target web --no-typescript --out-dir wasm/target \
        target/wasm32-unknown-unknown/release/sector.wasm
