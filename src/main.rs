extern crate crypto;

use std::cmp::Ordering;
use std::env;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Error, ErrorKind,
              Seek, SeekFrom, Read, Write};
use std::path::Path;
use std::ffi::OsStr;

use crypto::digest::Digest;
use crypto::sha1::Sha1;

const HASH_SIZE: usize = 20;
const SHA1_EMPTY: [u8; HASH_SIZE] = [218,  57,   163,  238,  94, 
                                     107,  75,   13,   50,   85, 
                                     191,  239,  149,  96,   24, 
                                     144,  175,  216,  7,    9];
const STATE_CACHE_VERSION: u32 = 3;

struct DxvkStateCacheHeader {
    magic:      [u8; 4],
    version:    u32,
    entry_size: usize
}

#[derive(Clone)]
struct DxvkStateCacheEntry {
    data: Vec<u8>,
    hash: Vec<u8>
}

impl Ord for DxvkStateCacheEntry {
    fn cmp(&self, other: &DxvkStateCacheEntry) -> Ordering {
        let sum_a =  self.hash.iter()
                         .rev().fold(0, |a, &b| a * 2 + u32::from(b));
        let sum_b = other.hash.iter()
                         .rev().fold(0, |a, &b| a * 2 + u32::from(b));
        sum_a.cmp(&sum_b)
    }
}

impl PartialOrd for DxvkStateCacheEntry {
    fn partial_cmp(&self, other: &DxvkStateCacheEntry) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for DxvkStateCacheEntry {
    fn eq(&self, other: &DxvkStateCacheEntry) -> bool {
        self.hash == other.hash
    }
}

impl Eq for DxvkStateCacheEntry { }

impl DxvkStateCacheEntry {
    fn with_capacity(capacity: usize) -> DxvkStateCacheEntry {
        DxvkStateCacheEntry {
            data: vec![0; capacity - HASH_SIZE],
            hash: vec![0; HASH_SIZE]
        }
    }

    fn compute_hash(&self) -> [u8; 20] {
        let mut hasher = Sha1::new();
        hasher.input(&self.data);
        hasher.input(&SHA1_EMPTY);
        let mut computed_hash = [0; 20];
        hasher.result(&mut computed_hash);

        computed_hash
    }

    fn is_valid(&self) -> bool {
        self.compute_hash() == *self.hash
    }

    fn convert_v2(&mut self) {
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
}

trait ReadEx: Read {
    fn read_u32(&mut self) -> io::Result<u32> {
        let mut buf = [0; 4];
        match self.read(&mut buf) {
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
}
impl<R: Read> ReadEx for BufReader<R> {}

trait WriteEx: Write { 
    fn write_u32(&mut self, n: u32) -> io::Result<()> {
        let mut buf = [0; 4];
        buf[0] =  n        as u8;
        buf[1] = (n >> 8)  as u8;
        buf[2] = (n >> 16) as u8;
        buf[3] = (n >> 24) as u8;
        self.write_all(&buf)
    }
}
impl<W: Write> WriteEx for BufWriter<W> { }

fn main() -> Result<(), io::Error> {
    let mut args: Vec<String> = env::args().collect();
    if env::args().any(|x| x == "--help" || x == "-h")
    || env::args().len() <= 1 {
        println!("Standalone dxvk-cache merger");
        println!("USAGE:\n\tdxvk-cache-tool [OPTION]... [FILE]...\n");
        println!("OPTIONS:\n\t-o, --output\tOutput file");
        return Ok(());
    }

    let output_path = match env::args().position(|x| x == "--output" 
                                                  || x == "-o") {
        Some(p) => {
            match env::args().nth(p + 1) {
                Some(s) => {
                    args.remove(p);
                    args.remove(p);
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

    let mut entries = Vec::new();
    for arg in args.into_iter().skip(1) {
        println!("Importing {}", arg);
        let path = Path::new(&arg);

        if !path.exists() {
            return Err(Error::new(ErrorKind::NotFound, 
                "File does not exists"));
        }

        if path.extension().is_some() 
        && path.extension().and_then(OsStr::to_str) == Some(".dxvk-cache") {
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

        if header.version != STATE_CACHE_VERSION {
            if header.version == 2 {
                println!("Converting outdated cache version {}",
                    &header.version);
            } else {
                return Err(Error::new(ErrorKind::InvalidData, 
                    format!("Unsupported cache version {}", header.version)))
            }
        };

        let mut entry = DxvkStateCacheEntry::with_capacity(
            header.entry_size
        );
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
            if header.version == STATE_CACHE_VERSION {
                match reader.read_exact(&mut entry.hash) {
                    Ok(_)   =>  (),
                    Err(e)  =>  return Err(e)
                };
            } else {
                entry.convert_v2();
                entry.hash = entry.compute_hash().to_vec();
                reader.seek(SeekFrom::Current(HASH_SIZE as i64))?;
            }
            if entry.is_valid() {
                entries.push(entry.clone());
            }
        }
    }

    if entries.is_empty() {
        return Err(Error::new(ErrorKind::Other, 
            "No valid cache entries found"));
    }

    entries.sort();
    entries.dedup();

    let file = File::create(&output_path)?;
    let mut writer = BufWriter::new(file);
    let header = DxvkStateCacheHeader {
        magic:      b"DXVK".to_owned(),
        version:    STATE_CACHE_VERSION,
        entry_size: {
            entries.first().map(
                |e| e.data.len() + e.hash.len()
            ).expect("And now... the darkness holds dominion – black as death")
        }
    };

    writer.write_all(&header.magic)?;
    writer.write_u32(header.version)?;
    writer.write_u32(header.entry_size as u32)?;
    for entry in &entries {
        writer.write_all(&entry.data)?;
        writer.write_all(&entry.hash)?;
    }

    println!("Merged cache {} contains {} entries",
        &output_path, 
        entries.len());
    Ok(())
}