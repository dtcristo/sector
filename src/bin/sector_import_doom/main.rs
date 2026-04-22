mod importer;

use importer::import_doom_map;
use sector::map::save_map_to_path;

use std::{env, path::PathBuf, process::ExitCode};

struct Args {
    wad_path: PathBuf,
    map_name: String,
    output_path: PathBuf,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = env::args_os();
        let _ = args.next();

        let wad_path = args.next().map(PathBuf::from).ok_or_else(Self::usage)?;
        let map_name = args
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or_else(Self::usage)?;
        let output_path = args
            .next()
            .map(PathBuf::from)
            .unwrap_or_else(|| default_output_path(&map_name));

        if args.next().is_some() {
            return Err(Self::usage());
        }

        Ok(Self {
            wad_path,
            map_name,
            output_path,
        })
    }

    fn usage() -> String {
        "usage: cargo run --bin sector_import_doom -- <wad-path> <map-id> [output-path]".into()
    }
}

fn default_output_path(map_name: &str) -> PathBuf {
    PathBuf::from("assets")
        .join("maps")
        .join(format!("{}.map.pb", map_name.to_ascii_lowercase()))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse()
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    let imported = import_doom_map(&args.wad_path, &args.map_name)?;
    save_map_to_path(&imported.map, &args.output_path)?;

    println!(
        "wrote {} with {} generated sectors from {} Doom sectors ({} sky sectors)",
        args.output_path.display(),
        imported.generated_sector_count,
        imported.doom_sector_count,
        imported.sky_sector_count
    );

    Ok(())
}
