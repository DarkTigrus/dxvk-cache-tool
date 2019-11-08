mod dxvk;
mod error;
mod util;

use dxvk::DxvkStateCache;
use error::{Error, ErrorKind};
use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;

struct Config {
    files:   Vec<PathBuf>,
    output:  PathBuf,
    version: u32
}

impl Default for Config {
    fn default() -> Self {
        Config {
            files:   Vec::new(),
            output:  PathBuf::from("output.dxvk-cache"),
            version: 0
        }
    }
}

fn print_help() {
    println!("Standalone dxvk-cache merger");
    println!("USAGE:\n\tdxvk-cache-tool [OPTION]... <FILEs>...\n");
    println!("OPTIONS:");
    println!("\t-o, --output FILE\tSet output file name");
    println!("\t-h, --help\t\tDisplay this help and exit");
    println!("\t-V, --version\t\tOutput version information and exit");
}

fn process_args() -> Config {
    let mut config = Config::default();
    let mut args: Vec<String> = env::args().collect();
    for (i, arg) in env::args().enumerate().rev() {
        match arg.as_ref() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            },
            "-o" | "--output" => {
                config.output = PathBuf::from(&args[i + 1]);
                args.drain(i..=i + 1);
            },
            "-V" | "--version" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            },
            "--frog" => {
                println!("🐸");
                std::process::exit(0);
            },
            _ => ()
        }
    }
    if args.len() <= 1 {
        print_help();
        std::process::exit(0);
    }
    args.remove(0);
    for arg in args {
        config.files.push(PathBuf::from(arg));
    }
    config
}

fn main() -> Result<(), Error> {
    let mut config = process_args();

    print!("Merging files");
    for path in config.files.iter() {
        print!(" {}", path.file_name().and_then(OsStr::to_str).unwrap());
    }
    println!();

    let mut state_cache = DxvkStateCache::new();
    for (i, path) in config.files.iter().enumerate() {
        if path.extension().and_then(OsStr::to_str) != Some("dxvk-cache") {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "File extension mismatch: expected .dxvk-cache"
            ));
        }

        let _state_cache = DxvkStateCache::open(path)?;
        if config.version == 0 {
            config.version = _state_cache.header.version;
            state_cache.header = _state_cache.header;
            println!(
                "Detected state cache version v{}",
                _state_cache.header.version
            );
        }

        let new_count = state_cache.extend(_state_cache)?;
        println!(
            "Merging {} ({}/{})... {} new entries",
            path.file_name().and_then(OsStr::to_str).unwrap(),
            i + 1,
            config.files.len(),
            new_count
        );
    }

    if state_cache.entries.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "No valid state cache entries found"
        ));
    }

    println!(
        "Writing {} entries to file {}",
        state_cache.entries.len(),
        config.output.file_name().and_then(OsStr::to_str).unwrap()
    );

    state_cache.save(config.output)?;
    println!("Finished");

    Ok(())
}
