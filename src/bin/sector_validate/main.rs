use sector::map::load_map_from_path;
use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _ = args.next();
    let Some(path) = args.next() else {
        eprintln!("usage: cargo run --bin sector_validate -- <map-path>");
        return ExitCode::FAILURE;
    };

    let path = PathBuf::from(path);
    if args.next().is_some() {
        eprintln!("sector_validate accepts exactly one map path");
        return ExitCode::FAILURE;
    }

    match load_map_from_path(&path) {
        Ok(_) => {
            println!("map valid: {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("map invalid: {}: {error}", path.display());
            ExitCode::FAILURE
        }
    }
}
