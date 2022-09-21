run:
    cargo run --release

build:
    cargo build --release

serve: build_web
    miniserve --index index.html wasm

build_web:
    cargo build --release --target wasm32-unknown-unknown
    wasm-bindgen --target web --out-dir wasm/target \
        target/wasm32-unknown-unknown/release/sector.wasm
