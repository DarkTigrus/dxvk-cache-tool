use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, prelude::*, BufReader, BufWriter};
use std::path::Path;

use linked_hash_map::LinkedHashMap;
use sha1::Sha1;

const HASH_SIZE: usize = 20;
const MAGIC_STRING: [u8; 4] = *b"DXVK";
const SHA1_EMPTY: [u8; HASH_SIZE] = [
    218, 57, 163, 238, 94, 107, 75, 13, 50, 85, 191, 239, 149, 96, 24, 144, 175, 216, 7, 9
];

struct Config {
    output:     String,
    version:    u32,
    entry_size: usize,
    files:      Vec<String>
}

impl Default for Config {
    fn default() -> Self {
        Config {
            output:     "output.dxvk-cache".to_owned(),
            version:    0,
            entry_size: 0,
            files:      Vec::new()
        }
    }
}

struct DxvkStateCacheHeader {
    magic:      [u8; 4],
    version:    u32,
    entry_size: usize
}

struct DxvkStateCacheEntry {
    data: Vec<u8>,
    hash: [u8; HASH_SIZE]
}

impl DxvkStateCacheEntry {
    fn with_length(length: usize) -> Self {
        DxvkStateCacheEntry {
            data: vec![0; length],
            hash: [0; HASH_SIZE]
        }
    }

    fn is_valid(&self) -> bool {
        let mut hasher = Sha1::default();
        hasher.update(&self.data);
        hasher.update(&SHA1_EMPTY);
        let hash = hasher.digest().bytes();

        hash == self.hash
    }
}

#[derive(Debug)]
enum ErrorKind {
    IoError,
    InvalidInput,
    InvalidData
}

#[derive(Debug)]
struct AppError {
    kind:    ErrorKind,
    message: String
}

impl AppError {
    fn new<S: Into<String>>(kind: ErrorKind, message: S) -> Self {
        AppError {
            kind,
            message: message.into()
        }
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        AppError {
            kind:    ErrorKind::IoError,
            message: error.to_string()
        }
    }
}

impl<R: Read> ReadEx for BufReader<R> {}
trait ReadEx: Read {
    fn read_u32(&mut self) -> io::Result<u32> {
        let mut buf = [0; 4];
        match self.read(&mut buf) {
            Ok(_) => Ok((u32::from(buf[0]))
                + (u32::from(buf[1]) << 8)
                + (u32::from(buf[2]) << 16)
                + (u32::from(buf[3]) << 24)),
            Err(e) => Err(e)
        }
    }
}

impl<W: Write> WriteEx for BufWriter<W> {}
trait WriteEx: Write {
    fn write_u32(&mut self, n: u32) -> io::Result<()> {
        let mut buf = [0; 4];
        buf[0] = n as u8;
        buf[1] = (n >> 8) as u8;
        buf[2] = (n >> 16) as u8;
        buf[3] = (n >> 24) as u8;
        self.write_all(&buf)
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
                config.output = args[i + 1].to_owned();
                args.drain(i..=i + 1);
            },
            "-V" | "--version" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
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
    config.files = args;
    config
}

fn main() -> Result<(), AppError> {
    let mut config = process_args();

    println!("Merging files {:?}", config.files);
    let mut entries = LinkedHashMap::new();
    for file in config.files {
        let path = Path::new(&file);
        println!("Importing file {:?}", path.file_name().unwrap());

        if path.extension().and_then(OsStr::to_str) != Some("dxvk-cache") {
            return Err(AppError::new(
                ErrorKind::InvalidInput,
                "File extension mismatch: expected .dxvk-cache"
            ));
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let header = DxvkStateCacheHeader {
            magic:      {
                let mut magic = [0; 4];
                reader.read_exact(&mut magic)?;
                magic
            },
            version:    reader.read_u32()?,
            entry_size: reader.read_u32()? as usize
        };

        if header.magic != MAGIC_STRING {
            return Err(AppError::new(
                ErrorKind::InvalidData,
                "Magic string mismatch"
            ));
        }

        if config.version == 0 {
            config.version = header.version;
            config.entry_size = header.entry_size;
            println!("Detected state cache version v{}", header.version);
        }

        if header.version != config.version {
            return Err(AppError::new(
                ErrorKind::InvalidInput,
                format!(
                    "State cache version mismatch: expected v{}, found v{}",
                    header.version, config.version
                )
            ));
        }

        let entries_len = entries.len();
        loop {
            let mut entry = DxvkStateCacheEntry::with_length(header.entry_size - HASH_SIZE);
            match reader.read_exact(&mut entry.data) {
                Ok(_) => (),
                Err(e) => {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    return Err(AppError::from(e));
                }
            };
            reader.read_exact(&mut entry.hash)?;
            if entry.is_valid() {
                entries.insert(entry.hash, entry.data);
            }
        }
        println!("Imported {} entries", entries.len() - entries_len);
    }

    if entries.is_empty() {
        return Err(AppError::new(
            ErrorKind::InvalidData,
            "No valid state cache entries found"
        ));
    }

    let file = File::create(&config.output)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(&MAGIC_STRING)?;
    writer.write_u32(config.version)?;
    writer.write_u32(config.entry_size as u32)?;
    for (hash, data) in &entries {
        writer.write_all(data)?;
        writer.write_all(hash)?;
    }

    println!(
        "Merged state cache {} contains {} entries",
        &config.output,
        entries.len()
    );
    Ok(())
}
