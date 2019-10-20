use sha1::Sha1;

pub type Sha1Hash = [u8; HASH_SIZE];
pub const LEGACY_VERSION: u32 = 7;
pub const HASH_SIZE: usize = 20;
pub const MAGIC_STRING: [u8; 4] = *b"DXVK";
const SHA1_EMPTY: Sha1Hash = [
    218, 57, 163, 238, 94, 107, 75, 13, 50, 85, 191, 239, 149, 96, 24, 144, 175, 216, 7, 9
];

#[derive(PartialEq)]
pub enum DxvkStateCacheEdition {
    Standard,
    Legacy
}

pub struct DxvkStateCacheHeader {
    pub magic:      [u8; 4],
    pub version:    u32,
    pub entry_size: u32
}

pub struct DxvkStateCacheEntryHeader {
    pub stage_mask: u8,
    pub entry_size: u32
}

pub struct DxvkStateCacheEntry {
    pub header: Option<DxvkStateCacheEntryHeader>,
    pub hash:   [u8; HASH_SIZE],
    pub data:   Vec<u8>
}

impl DxvkStateCacheEntry {
    pub fn with_length(length: usize) -> Self {
        DxvkStateCacheEntry {
            data:   vec![0; length - HASH_SIZE],
            hash:   [0; HASH_SIZE],
            header: None
        }
    }

    pub fn with_header(header: DxvkStateCacheEntryHeader) -> Self {
        DxvkStateCacheEntry {
            data:   vec![0; header.entry_size as usize],
            hash:   [0; HASH_SIZE],
            header: Some(header)
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
