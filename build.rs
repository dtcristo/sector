use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=assets/maps");

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
