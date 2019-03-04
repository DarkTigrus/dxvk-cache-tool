extern crate crypto;

use std::env;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, prelude::*};
use std::mem;
use std::path::Path;
use std::ffi::OsStr;

use crypto::digest::Digest;
use crypto::sha1::Sha1;

const SHA1_EMPTY: [u8; 20] = [218,  57,   163,  238,  94, 
                              107,  75,   13,   50,   85, 
                              191,  239,  149,  96,   24, 
                              144,  175,  216,  7,    9];
const STATE_CACHE_DATA_SIZE: usize = 1804;
const STATE_CACHE_VERSION: u32 = 3;

struct DxvkStateCacheHeader {
    magic:      [u8; 4],
    version:    u32,
    entry_size: u32
}

impl Default for DxvkStateCacheHeader {
    fn default() -> DxvkStateCacheHeader {
        DxvkStateCacheHeader {
            magic:      b"DXVK".to_owned(),
            version:    STATE_CACHE_VERSION,
            entry_size: mem::size_of::<DxvkStateCacheEntry>() as u32
        }
    }
}

#[derive(Copy, Clone)]
struct DxvkStateCacheEntry {
    data: [u8; STATE_CACHE_DATA_SIZE],
    hash: [u8; 20]
}

impl Default for DxvkStateCacheEntry {
    fn default() -> DxvkStateCacheEntry {
        DxvkStateCacheEntry {
            data: [0; STATE_CACHE_DATA_SIZE],
            hash: [0; 20]
        }
    }
}

impl PartialEq for DxvkStateCacheEntry {
    fn eq(&self, other: &DxvkStateCacheEntry) -> bool {
        self.hash == other.hash
    }
}

impl DxvkStateCacheEntry {
    fn is_valid(&self) -> bool {
        let mut hasher = Sha1::new();
        hasher.input(&self.data);
        hasher.input(&SHA1_EMPTY);
        let mut computed_hash = [0; 20];
        hasher.result(&mut computed_hash);

        computed_hash == self.hash
    }
}

fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut buf = [0; 4];
    match r.read(&mut buf) {
        Ok(_) => {
            Ok(
                (u32::from(buf[0])      ) +
                (u32::from(buf[1]) <<  8) +
                (u32::from(buf[2]) << 16) +
                (u32::from(buf[3]) << 24)
            )
        },
        Err(e) => Err(e)
    }
}

fn write_u32<W: Write>(w: &mut W, n: u32) -> io::Result<()> {
    let mut buf = [0; 4];
    buf[0] =  n        as u8;
    buf[1] = (n >> 8)  as u8;
    buf[2] = (n >> 16) as u8;
    buf[3] = (n >> 24) as u8;
    w.write_all(&buf)
}

fn main() -> Result<(), io::Error> {
    if env::args().any(|x| x == "--help" || x == "-h")
    || env::args().len() <= 1 {
        println!("USAGE: dxvk-cache-tool [FILE]...");
        return Ok(());
    }

    let mut entries = Vec::new();

    for arg in env::args().skip(1) {
        println!("Importing {}", arg);
        let path = Path::new(&arg);

        if !path.exists() {
            println!("File does not exists");
            continue;
        }

        if path.extension().is_some() 
        && path.extension().and_then(OsStr::to_str) == Some(".dxvk-cache") {
            println!("File extension mismatch");
            continue;
        }

        let file = File::open(path).expect("Unable to open file");
        let mut reader = BufReader::new(file);

        let header = DxvkStateCacheHeader {
            magic: {
                let mut magic = [0; 4];
                reader.read_exact(&mut magic)?;
                magic
            },
            version:    read_u32(&mut reader)?,
            entry_size: read_u32(&mut reader)?
        };

        if &header.magic != b"DXVK" {
            println!("Magic string mismatch");
            continue;
        }

        if header.version != STATE_CACHE_VERSION {
            println!("Unsupported cache version {}",
                    header.version);
            continue;
        }

        let mut entry = DxvkStateCacheEntry::default();
        loop {
            match reader.read_exact(&mut entry.data) {
                Ok(_)   =>  (),
                Err(e)  =>  {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        break
                    }
                    return Err(e);
                }
            };
            match reader.read_exact(&mut entry.hash) {
                Ok(_)   =>  (),
                Err(e)  =>  return Err(e)
            };
            if entry.is_valid() && !entries.contains(&entry) {
                entries.push(entry);
            }
        }
    }
    
    if entries.is_empty() {
        println!("No valid cache entries found");
        return Ok(());
    }

    let file = File::create("output.dxvk-cache")?;
    let mut writer = BufWriter::new(file);
    let header = DxvkStateCacheHeader::default();
    writer.write_all(&header.magic)?;
    write_u32(&mut writer, header.version)?;
    write_u32(&mut writer, header.entry_size)?;
    for entry in &entries {
        writer.write_all(&entry.data)?;
        writer.write_all(&entry.hash)?;
    }

    println!("Merged cache output.dxvk-cache contains {} entries",
        entries.len());
    Ok(())
}