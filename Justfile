play MAP_NAME="default":
    @map_path=$(if [ -f assets/maps/{{MAP_NAME}}.map.pb ]; then printf '%s' assets/maps/{{MAP_NAME}}.map.pb; elif [ -f assets/maps/{{MAP_NAME}}.map.ron ]; then printf '%s' assets/maps/{{MAP_NAME}}.map.ron; else printf '%s' assets/maps/{{MAP_NAME}}.map.ron; fi); \
        cargo run --bin sector --features "sector bevy/dynamic_linking" -- "$map_path"

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

validate MAP_NAME="default":
    @map_path=$(if [ -f assets/maps/{{MAP_NAME}}.map.pb ]; then printf '%s' assets/maps/{{MAP_NAME}}.map.pb; elif [ -f assets/maps/{{MAP_NAME}}.map.ron ]; then printf '%s' assets/maps/{{MAP_NAME}}.map.ron; else printf '%s' assets/maps/{{MAP_NAME}}.map.ron; fi); \
        cargo run --bin sector_validate -- "$map_path"

import-doom WAD_PATH MAP_ID:
    cargo run --bin sector_import_doom --features doom_import -- {{WAD_PATH}} {{MAP_ID}}

serve-web: build-web
    miniserve --index index.html --spa wasm

build-web:
    cargo build --bin sector --features sector --release --target wasm32-unknown-unknown

    rm -rf wasm/target/
    wasm-bindgen --target web --no-typescript --out-dir wasm/target \
        target/wasm32-unknown-unknown/release/sector.wasm

    rm -rf wasm/assets/
    cp -R assets wasm/
