extern crate sha1;
extern crate linked_hash_map;

use std::env;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Error, ErrorKind,
              Seek, SeekFrom, Read, Write};
use std::path::Path;
use std::ffi::OsStr;

use sha1::Sha1;

use linked_hash_map::LinkedHashMap;

const DATA_SIZE: usize = 1804;
const HASH_SIZE: usize = 20;
const SHA1_EMPTY: [u8; HASH_SIZE] = [218,  57,   163,  238,  94, 
                                     107,  75,   13,   50,   85, 
                                     191,  239,  149,  96,   24, 
                                     144,  175,  216,  7,    9];
const SUPPORTED_VERSIONS: [u32; 2] = [2, 3];
const DEFAULT_CACHE_VERSION: u32 = 3;

struct DxvkStateCacheHeader {
    magic:      [u8; 4],
    version:    u32,
    entry_size: usize
}

struct DxvkStateCacheEntry {
    data: [u8; DATA_SIZE],
    hash: [u8; HASH_SIZE]
}

impl DxvkStateCacheEntry {
    fn new() -> DxvkStateCacheEntry {
        DxvkStateCacheEntry {
            data: [0; DATA_SIZE],
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

    fn upgrade_to_v3(&mut self) {
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

    fn downgrade_to_v2(&mut self) {
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

trait ReadEx: Read {
    fn read_u32(&mut self) -> io::Result<u32> {
        let mut buf = [0; 4];
        match self.read(&mut buf) {
            Ok(_) => {
                Ok((u32::from(buf[0])      ) +
                   (u32::from(buf[1]) <<  8) +
                   (u32::from(buf[2]) << 16) +
                   (u32::from(buf[3]) << 24))
            },
            Err(e) => Err(e)
        }
    }
}
impl<R: Read> ReadEx for BufReader<R> {}

trait WriteEx: Write { 
    fn write_u32(&mut self, n: u32) -> io::Result<()> {
        let mut buf = [0; 4];
        buf[0] =  n        as u8;
        buf[1] = (n >> 8 ) as u8;
        buf[2] = (n >> 16) as u8;
        buf[3] = (n >> 24) as u8;
        self.write_all(&buf)
    }
}
impl<W: Write> WriteEx for BufWriter<W> { }

fn print_help() {
    println!("Standalone dxvk-cache merger");
    println!("USAGE:\n\tdxvk-cache-tool [OPTION]... [FILE]...\n");
    println!("OPTIONS:");
    println!("\t-o, --output [FILE]\tOutput file");
    println!("\t-t, --target [2,3]\tTarget version");
}

fn main() -> Result<(), io::Error> {
    let mut args: Vec<String> = env::args().collect();
    if env::args().any(|x| x == "--help" || x == "-h")
    || env::args().len() <= 1 {
        print_help();
        return Ok(());
    }

    let output = match env::args().position(|x| x == "-o"
                                             || x == "--output") {
        Some(pos) => {
            match env::args().nth(pos + 1) {
                Some(s) => {
                    args.drain(pos..pos + 2);
                    s
                },
                None => {
                    return Err(Error::new(ErrorKind::InvalidInput,
                        "Output file name argument is missing"));
                }
            }
        }
        None => "output.dxvk-cache".to_owned()
    };

    let target_version = match env::args().position(|x| x == "-t"
                                                     || x == "--target") {
        Some(pos) => {
            match env::args().nth(pos + 1) {
                Some(v) => {
                    args.drain(pos..pos + 2);
                    v.parse().expect("Not a number")
                },
                None => {
                    return Err(Error::new(ErrorKind::InvalidInput,
                        "Output file name argument is missing"));
                }
            }
        }
        None => DEFAULT_CACHE_VERSION
    };
    if !SUPPORTED_VERSIONS.contains(&target_version) {
        return Err(Error::new(ErrorKind::InvalidData,
                format!("Unsupported target version {}", target_version)))
    };

    let mut entries = LinkedHashMap::new();
    for arg in args.into_iter().skip(1) {
        println!("Importing {}", arg);
        let path = Path::new(&arg);

        if !path.exists() {
            return Err(Error::new(ErrorKind::NotFound, 
                "File does not exists"));
        }

        if path.extension().and_then(OsStr::to_str) != Some("dxvk-cache") {
            return Err(Error::new(ErrorKind::InvalidInput, 
                "File extension mismatch"));
        }

        let file = File::open(path).expect("Unable to open file");
        let mut reader = BufReader::new(file);

        let header = DxvkStateCacheHeader {
            magic: {
                let mut magic = [0; 4];
                reader.read_exact(&mut magic)?;
                magic
            },
            version:    reader.read_u32()?,
            entry_size: reader.read_u32()? as usize
        };

        if &header.magic != b"DXVK" {
            return Err(Error::new(ErrorKind::InvalidData, 
                "Magic string mismatch"));
        }

        if !SUPPORTED_VERSIONS.contains(&header.version) {
            return Err(Error::new(ErrorKind::InvalidData,
                    format!("Unsupported cache version {}", header.version)))
        };

        if header.version != target_version {
            match header.version {
                v if v > target_version => {
                    println!("Downgrading to version {}", target_version)
                },
                v if v < target_version => {
                    println!("Upgrading to version {}", target_version)
                },
                _   => ()
            }
        }

        assert!(header.entry_size == DATA_SIZE + HASH_SIZE);
        
        loop {
            let mut entry = DxvkStateCacheEntry::new();
            match reader.read_exact(&mut entry.data) {
                Ok(_)   =>  (),
                Err(e)  =>  {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        break
                    }
                    return Err(e);
                }
            };
            if target_version == header.version {
                match reader.read_exact(&mut entry.hash) {
                    Ok(_)   =>  (),
                    Err(e)  =>  return Err(e)
                };
            } else {
                match target_version {
                    3 => entry.upgrade_to_v3(),
                    2 => entry.downgrade_to_v2(),
                    _ => panic!(format!("Unexected cache version {}",
                                    header.version))
                }
                entry.hash = entry.compute_hash();
                reader.seek(SeekFrom::Current(HASH_SIZE as i64))?;
            }
            if entry.is_valid() {
                entries.insert(entry.hash, entry.data);
            }
        }
    }

    if entries.is_empty() {
        return Err(Error::new(ErrorKind::Other, 
            "No valid cache entries found"));
    }

    let file = File::create(&output)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(b"DXVK")?;
    writer.write_u32(target_version)?;
    writer.write_u32((DATA_SIZE + HASH_SIZE) as u32)?;
    for entry in &entries {
        writer.write_all(entry.1)?;
        writer.write_all(entry.0)?;
    }

    println!("Merged cache {} contains {} entries",
        &output, 
        entries.len());
    Ok(())
}