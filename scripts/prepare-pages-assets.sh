#!/usr/bin/env bash
set -euo pipefail

assets_dir=${1:-dist}
wasm_file="$assets_dir/target/sector_bg.wasm"
maps_dir="$assets_dir/assets/maps"
headers_file="$assets_dir/_headers"

if ! command -v brotli >/dev/null 2>&1; then
    printf 'Brotli must be installed before preparing Pages assets\n' >&2
    exit 1
fi

if [[ ! -f "$wasm_file" ]]; then
    printf 'Missing WebAssembly asset: %s\n' "$wasm_file" >&2
    exit 1
fi

if [[ ! -d "$maps_dir" ]]; then
    printf 'Missing shipped maps directory: %s\n' "$maps_dir" >&2
    exit 1
fi

shopt -s nullglob
map_files=("$maps_dir"/*.map.pb)
if ((${#map_files[@]} == 0)); then
    printf 'No binary map assets found in %s\n' "$maps_dir" >&2
    exit 1
fi

compress_asset() {
    local asset=$1
    local compressed="$asset.br.tmp"

    if brotli --test "$asset" >/dev/null 2>&1; then
        return
    fi

    brotli --quality=11 --output="$compressed" "$asset"
    mv "$compressed" "$asset"
}

compress_asset "$wasm_file"
for map_file in "${map_files[@]}"; do
    compress_asset "$map_file"
done

compressed_bytes=$(wc -c < "$wasm_file" | tr -d '[:space:]')
max_asset_bytes=$((25 * 1024 * 1024))
if ((compressed_bytes > max_asset_bytes)); then
    printf 'Brotli WebAssembly asset exceeds Cloudflare’s 25 MiB limit: %s bytes\n' "$compressed_bytes" >&2
    exit 1
fi

cat > "$headers_file" <<'EOF'
/target/sector_bg.wasm
  Content-Encoding: br
  Content-Type: application/wasm
/assets/maps/*.map.pb
  Content-Encoding: br
  Content-Type: application/octet-stream
EOF
