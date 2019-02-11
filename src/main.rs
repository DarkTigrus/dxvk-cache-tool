extern crate crypto;

use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, BufReader, Read};
use std::mem;
use std::path::Path;
use std::slice;
use std::ffi::OsStr;

use crypto::digest::Digest;
use crypto::sha1::Sha1;

const SHA1_EMPTY: [u8; 20] = [218,  57,   163,  238,  94, 
                              107,  75,   13,   50,   85, 
                              191,  239,  149,  96,   24, 
                              144,  175,  216,  7,    9];
const STATE_CACHE_DATA_SIZE: usize = 1804;
const STATE_CACHE_VERSION: u32 = 3;

#[repr(C)]
#[allow(dead_code)]
#[derive(Copy, Clone)]
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

#[repr(C)]
#[allow(dead_code)]
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
    fn is_valid(mut self) -> bool {
        let expected_hash = self.hash;
        self.hash = SHA1_EMPTY;

        let mut hasher = Sha1::new();
        hasher.input(any_as_u8_slice(&self));
        let mut computed_hash = [0u8; 20];
        hasher.result(&mut computed_hash);

        self.hash = expected_hash;
        computed_hash == expected_hash
    }
}

fn read_into_type<T, R: Read>(read: &mut R, data: &mut T)
    -> io::Result<()> {
    let mut buffer = unsafe { 
        slice::from_raw_parts_mut(
            &mut *data as *mut T as *mut u8,
            std::mem::size_of::<T>()
    )};
    read.read_exact(&mut buffer)
}

fn any_as_u8_slice<T>(p: &T) -> &[u8] {
    unsafe {
        slice::from_raw_parts(
            p as *const _ as *const u8,
            std::mem::size_of::<T>()
    )}
}

fn main() {
    if env::args().any(|x| x == "--help" || x == "-h")
    || env::args().len() == 1 {
        println!("USAGE: dxvk-cache-tool [FILE]...");
        return;
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
        let mut buf_reader = BufReader::new(file);

        let mut header = DxvkStateCacheHeader::default();
        read_into_type(&mut buf_reader, &mut header).unwrap();

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
        while read_into_type(&mut buf_reader, &mut entry).is_ok() {
            if entry.is_valid() && !entries.contains(&entry) {
                entries.push(entry);
            }
        }
    }
    
    if entries.is_empty() {
        println!("No valid cache entries found");
        return;
    }

    let header = DxvkStateCacheHeader::default();
    let mut buffer = File::create("output.dxvk-cache").unwrap();
    buffer.write_all(any_as_u8_slice(&header)).unwrap();
    for entry in &entries {
        buffer.write_all(
            any_as_u8_slice(entry)
        ).expect("Unable to write buffer");
    }

    println!("Merged cache output.dxvk-cache contains {} entries",
        entries.len());
}