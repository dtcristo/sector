use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=assets/maps");
    println!("cargo:rerun-if-changed=proto/sector_map.proto");

    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc should exist");
    let protoc_dir = protoc
        .parent()
        .expect("vendored protoc path should have a parent directory");
    let existing_path = env::var_os("PATH").unwrap_or_default();
    let mut path_entries = env::split_paths(&existing_path).collect::<Vec<_>>();
    path_entries.insert(0, protoc_dir.to_path_buf());
    let updated_path = env::join_paths(path_entries).expect("joined PATH should be valid");
    env::set_var("PATH", updated_path);

    protobuf_codegen::CodeGen::new()
        .include("proto")
        .input("sector_map.proto")
        .output_dir(
            PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR should exist")).join("map_proto"),
        )
        .generate_and_compile()
        .expect("failed to generate protobuf map bindings");

    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should exist"));
    let maps_dir = manifest_dir.join("assets").join("maps");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR should exist"));

    let mut maps = fs::read_dir(&maps_dir)
        .expect("assets/maps should exist")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_file() {
                return None;
            }

            let file_name = path.file_name()?.to_string_lossy();
            let (map_name, _) = file_name.split_once(".map.")?;
            let asset_path = format!("assets/maps/{file_name}");
            Some((map_name.to_string(), asset_path, path))
        })
        .collect::<Vec<_>>();
    maps.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let mut generated = String::from("static EMBEDDED_MAPS: &[EmbeddedMap] = &[\n");
    for (_map_name, asset_path, source_path) in maps {
        generated.push_str(&format!(
            "    EmbeddedMap {{ asset_path: {asset_path:?}, bytes: include_bytes!({source_path:?}) }},\n"
        ));
    }
    generated.push_str("];\n");

    fs::write(out_dir.join("embedded_maps.rs"), generated)
        .expect("failed to write embedded map registry");
}
