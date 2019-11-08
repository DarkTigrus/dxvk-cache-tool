use super::util::ReadEx;
use super::{Error, ErrorKind};
use linked_hash_map::LinkedHashMap;
use sha1::Sha1;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

type Sha1Hash = [u8; HASH_SIZE];
const LEGACY_VERSION: u32 = 7;
const HASH_SIZE: usize = 20;
const MAGIC_STRING: [u8; 4] = *b"DXVK";
const SHA1_EMPTY: Sha1Hash = [
    218, 57, 163, 238, 94, 107, 75, 13, 50, 85, 191, 239, 149, 96, 24, 144, 175, 216, 7, 9
];

pub struct DxvkStateCache {
    pub header:  DxvkStateCacheHeader,
    pub entries: LinkedHashMap<Sha1Hash, DxvkStateCacheEntry>
}

impl DxvkStateCache {
    pub fn new() -> Self {
        Self {
            header:  DxvkStateCacheHeader {
                magic:      MAGIC_STRING,
                version:    0,
                entry_size: 0
            },
            entries: LinkedHashMap::new()
        }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        fn read_entry<R: Read>(
            reader: &mut BufReader<R>
        ) -> Result<DxvkStateCacheEntry, io::Error> {
            let header = DxvkStateCacheEntryHeader::new(reader.read_u32()?);
            let mut entry = DxvkStateCacheEntry::with_header(header);
            reader.read_exact(&mut entry.hash)?;
            reader.read_exact(&mut entry.data)?;
            Ok(entry)
        }

        fn read_entry_v7<R: Read>(
            reader: &mut BufReader<R>,
            size: usize
        ) -> Result<DxvkStateCacheEntry, io::Error> {
            let mut entry = DxvkStateCacheEntry::with_length(size);
            reader.read_exact(&mut entry.data)?;
            reader.read_exact(&mut entry.hash)?;
            Ok(entry)
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut entries = LinkedHashMap::new();
        let header = DxvkStateCacheHeader {
            magic:      reader.read_u32()?.to_le_bytes(),
            version:    reader.read_u32()?,
            entry_size: reader.read_u32()?
        };

        if header.magic != MAGIC_STRING {
            return Err(Error::new(ErrorKind::InvalidData, "Magic string mismatch"));
        }

        loop {
            let result = if header.version > LEGACY_VERSION {
                read_entry(&mut reader)
            } else {
                read_entry_v7(&mut reader, header.entry_size as usize)
            };

            let entry = match result {
                Ok(e) => e,
                Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(Error::from(e))
            };

            if entry.is_valid() {
                entries.insert(entry.hash, entry);
            }
        }

        Ok(Self {
            header,
            entries
        })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&self.header.magic)?;
        writer.write_all(&self.header.version.to_le_bytes())?;
        writer.write_all(&self.header.entry_size.to_le_bytes())?;
        for (_, entry) in &self.entries {
            if let Some(header) = &entry.header {
                writer.write_all(&header.stage_mask.to_le_bytes())?;
                writer.write_all(&header.entry_size)?;
                writer.write_all(&entry.hash)?;
                writer.write_all(&entry.data)?;
            } else {
                writer.write_all(&entry.data)?;
                writer.write_all(&entry.hash)?;
            }
        }

        Ok(())
    }

    pub fn extend(&mut self, other: Self) -> Result<usize, Error> {
        if self.header.version != other.header.version {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "State cache version mismatch: expected v{}, found v{}",
                    self.header.version, other.header.version
                )
            ));
        }

        let len = self.entries.len();
        self.entries.extend(other.entries);
        Ok(self.entries.len() - len)
    }
}

#[derive(Copy, Clone)]
pub struct DxvkStateCacheHeader {
    pub magic:      [u8; 4],
    pub version:    u32,
    pub entry_size: u32
}

#[derive(Copy, Clone)]
pub struct DxvkStateCacheEntryHeader {
    pub stage_mask: u8,
    entry_size:     [u8; 3]
}

impl DxvkStateCacheEntryHeader {
    pub fn new(n: u32) -> Self {
        let stage_mask = n as u8;
        let mut entry_size = [0; 3];
        entry_size[0] = (n >> 8) as u8;
        entry_size[1] = (n >> 16) as u8;
        entry_size[2] = (n >> 24) as u8;

        DxvkStateCacheEntryHeader {
            stage_mask,
            entry_size
        }
    }

    pub fn entry_size(self) -> u32 {
        u32::from_le_bytes([
            self.entry_size[0],
            self.entry_size[1],
            self.entry_size[2],
            0
        ])
    }
}

pub struct DxvkStateCacheEntry {
    pub header: Option<DxvkStateCacheEntryHeader>,
    pub hash:   Sha1Hash,
    pub data:   Vec<u8>
}

impl DxvkStateCacheEntry {
    pub fn with_length(length: usize) -> Self {
        Self {
            header: None,
            hash:   [0; HASH_SIZE],
            data:   vec![0; length - HASH_SIZE]
        }
    }

    pub fn with_header(header: DxvkStateCacheEntryHeader) -> Self {
        Self {
            header: Some(header),
            hash:   [0; HASH_SIZE],
            data:   vec![0; header.entry_size() as usize]
        }
    }

    pub fn is_valid(&self) -> bool {
        let mut hasher = Sha1::default();
        hasher.update(&self.data);
        if self.header.is_none() {
            hasher.update(&SHA1_EMPTY);
        }
        let hash = hasher.digest().bytes();

        hash == self.hash
    }
}
