use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, prelude::*, BufReader, BufWriter, Error, ErrorKind, SeekFrom};
use std::path::Path;

use linked_hash_map::LinkedHashMap;
use sha1::Sha1;

const SUPPORTED_VERSIONS: [u32; 5] = [2, 3, 4, 5, 6];
const DEFAULT_CACHE_VERSION: u32 = 6;
const DATA_SIZE_V2: usize = 1804;
const DATA_SIZE_V5: usize = 1836;
const DATA_SIZE_V6: usize = 1868;
const HASH_SIZE: usize = 20;
const MAGIC_STRING: [u8; 4] = *b"DXVK";
const SHA1_EMPTY: [u8; HASH_SIZE] = [
    218, 57, 163, 238, 94, 107, 75, 13, 50, 85, 191, 239, 149, 96, 24, 144, 175, 216, 7, 9
];

struct Config {
    output:  String,
    version: u32,
    files:   Vec<String>
}

impl Default for Config {
    fn default() -> Self {
        Config {
            output:  "output.dxvk-cache".to_owned(),
            version: DEFAULT_CACHE_VERSION,
            files:   Vec::new()
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

    fn compute_hash(&self) -> [u8; HASH_SIZE] {
        let mut hasher = Sha1::default();
        hasher.update(&self.data);
        hasher.update(&SHA1_EMPTY);
        hasher.digest().bytes()
    }

    fn is_valid(&self) -> bool {
        self.compute_hash() == self.hash
    }

    fn convert(&mut self, current: u32, target: u32) {
        let result = if current < target {
            match current {
                5 => unimplemented!("Upgrading to version 6 is not supported"),
                4 => unimplemented!("Upgrading to version 5 is not supported"),
                3 => self.upgrade_v3_to_v4(),
                2 => self.upgrade_v2_to_v3(),
                _ => unreachable!()
            }
            current + 1
        } else {
            match current {
                6 => unimplemented!("Downgrading to version 5 is not supported"),
                5 => unimplemented!("Downgrading to version 4 is not supported"),
                4 => self.downgrade_v4_to_v3(),
                3 => self.downgrade_v3_to_v2(),
                _ => unreachable!()
            }
            current - 1
        };

        if result != target {
            self.convert(result, target);
        }
    }

    fn upgrade_v3_to_v4(&mut self) {
        static OFFSET: usize = 1244;

        // xsAlphaCompareOp = VK_COMPARE_OP_ALWAYS
        if let Some(e) = self.data.get_mut(OFFSET) {
            assert!(*e == 0);
            *e = 7;
        }
    }

    fn downgrade_v4_to_v3(&mut self) {
        static OFFSET: usize = 1244;

        // xsAlphaCompareOp = VK_COMPARE_OP_NEVER
        if let Some(e) = self.data.get_mut(OFFSET) {
            assert!(*e == 7);
            *e = 0;
        }
    }

    fn upgrade_v2_to_v3(&mut self) {
        static OFFSET_1: usize = 1204;
        static OFFSET_2: usize = 1208;

        // rsDepthClipEnable = !rsDepthClipEnable;
        if let Some(e) = self.data.get_mut(OFFSET_1) {
            assert!(*e == 0 || *e == 1);
            *e = if *e == 0 { 1 } else { 0 };
        }
        // rsDepthBiasEnable = VK_FALSE;
        if let Some(e) = self.data.get_mut(OFFSET_2) {
            assert!(*e == 0 || *e == 1);
            *e = 0;
        }
    }

    fn downgrade_v3_to_v2(&mut self) {
        static OFFSET_1: usize = 1204;
        static OFFSET_2: usize = 1208;

        let mut enable_depth_bias = false;

        if let Some(e) = self.data.get_mut(OFFSET_1) {
            assert!(*e == 0 || *e == 1);
            *e = if *e == 0 {
                1
            } else {
                enable_depth_bias = true;
                0
            };
        }
        if enable_depth_bias {
            if let Some(e) = self.data.get_mut(OFFSET_2) {
                assert!(*e == 0 || *e == 1);
                *e = 1;
            }
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
    println!("USAGE:\n\tdxvk-cache-tool [OPTION]... <FILE>...\n");
    println!("OPTIONS:");
    println!("\t-o, --output FILE\tOutput file");
    println!("\t-t, --target [2-6]\tTarget version");
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
            "-t" | "--target" => {
                config.version = args[i + 1].parse().expect("Not a number");
                args.drain(i..=i + 1);
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

fn main() -> Result<(), io::Error> {
    let config = process_args();

    if !SUPPORTED_VERSIONS.contains(&config.version) {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Unsupported target version {}", config.version)
        ));
    };

    let mut entries = LinkedHashMap::new();
    for file in config.files {
        println!("Importing {}", file);
        let path = Path::new(&file);

        if !path.exists() {
            return Err(Error::new(ErrorKind::NotFound, "File does not exists"));
        }

        if path.extension().and_then(OsStr::to_str) != Some("dxvk-cache") {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "File extension mismatch"
            ));
        }

        let file = File::open(path).expect("Unable to open file");
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
            return Err(Error::new(ErrorKind::InvalidData, "Magic string mismatch"));
        }

        if !SUPPORTED_VERSIONS.contains(&header.version) {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unsupported cache version {}", header.version)
            ));
        };

        match header.version {
            v if v > config.version => println!("Downgrading to version {}", config.version),
            v if v < config.version => println!("Upgrading to version {}", config.version),
            _ => ()
        }

        loop {
            let mut entry = DxvkStateCacheEntry::with_length(header.entry_size - HASH_SIZE);
            match reader.read_exact(&mut entry.data) {
                Ok(_) => (),
                Err(e) => {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    return Err(e);
                }
            };
            if config.version == header.version {
                match reader.read_exact(&mut entry.hash) {
                    Ok(_) => (),
                    Err(e) => return Err(e)
                };
            } else {
                entry.convert(header.version, config.version);
                entry.hash = entry.compute_hash();
                reader.seek(SeekFrom::Current(HASH_SIZE as i64))?;
            }
            if entry.is_valid() {
                entries.insert(entry.hash, entry.data);
            }
        }
    }

    if entries.is_empty() {
        return Err(Error::new(ErrorKind::Other, "No valid cache entries found"));
    }

    let entry_size = match config.version {
        6 => DATA_SIZE_V6 + HASH_SIZE,
        5 => DATA_SIZE_V5 + HASH_SIZE,
        _ => DATA_SIZE_V2 + HASH_SIZE
    };

    let file = File::create(&config.output)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(&MAGIC_STRING)?;
    writer.write_u32(config.version)?;
    writer.write_u32(entry_size as u32)?;
    for (hash, data) in &entries {
        writer.write_all(data)?;
        writer.write_all(hash)?;
    }

    println!(
        "Merged cache {} contains {} entries",
        &config.output,
        entries.len()
    );
    Ok(())
}
