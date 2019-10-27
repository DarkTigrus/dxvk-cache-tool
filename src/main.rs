mod dxvk;
mod error;

use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, prelude::*, BufReader, BufWriter};
use std::path::PathBuf;

use dxvk::*;
use error::{Error, ErrorKind};
use linked_hash_map::LinkedHashMap;

struct Config {
    files:      Vec<PathBuf>,
    output:     PathBuf,
    entry_size: u32,
    version:    u32,
    edition:    DxvkStateCacheEdition
}

impl Default for Config {
    fn default() -> Self {
        Config {
            files:      Vec::new(),
            output:     PathBuf::from("output.dxvk-cache"),
            entry_size: 0,
            version:    0,
            edition:    DxvkStateCacheEdition::Standard
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

    fn read_u24(&mut self) -> io::Result<u32> {
        let mut buf = [0; 3];
        match self.read(&mut buf) {
            Ok(_) => Ok((u32::from(buf[0])) + (u32::from(buf[1]) << 8) + (u32::from(buf[2]) << 16)),
            Err(e) => Err(e)
        }
    }

    fn read_u8(&mut self) -> io::Result<u8> {
        let mut buf = [0; 1];
        match self.read(&mut buf) {
            Ok(_) => Ok(buf[0]),
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

    fn write_u24(&mut self, n: u32) -> io::Result<()> {
        let mut buf = [0; 3];
        buf[0] = n as u8;
        buf[1] = (n >> 8) as u8;
        buf[2] = (n >> 16) as u8;
        self.write_all(&buf)
    }

    fn write_u8(&mut self, n: u8) -> io::Result<()> {
        let mut buf = [0; 1];
        buf[0] = n;
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
    let mut entries = LinkedHashMap::new();
    for (i, path) in config.files.iter().enumerate() {
        if path.extension().and_then(OsStr::to_str) != Some("dxvk-cache") {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "File extension mismatch: expected .dxvk-cache"
            ));
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let header = read_header(&mut reader)?;

        if header.magic != MAGIC_STRING {
            return Err(Error::new(ErrorKind::InvalidData, "Magic string mismatch"));
        }

        if config.version == 0 {
            config.version = header.version;
            config.edition = if header.version > LEGACY_VERSION {
                DxvkStateCacheEdition::Standard
            } else {
                DxvkStateCacheEdition::Legacy
            };
            config.entry_size = header.entry_size;
            println!("Detected state cache version v{}", header.version);
        }

        if header.version != config.version {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "State cache version mismatch: expected v{}, found v{}",
                    config.version, header.version
                )
            ));
        }

        let mut omitted = 0;
        let entries_len = entries.len();
        print!(
            "Merging {} ({}/{})... ",
            path.file_name().and_then(OsStr::to_str).unwrap(),
            i + 1,
            config.files.len()
        );
        loop {
            let res = match config.edition {
                DxvkStateCacheEdition::Standard => read_entry(&mut reader),
                DxvkStateCacheEdition::Legacy => {
                    read_entry_legacy(&mut reader, header.entry_size as usize)
                },
            };
            match res {
                Ok(e) => {
                    if e.is_valid() {
                        entries.insert(e.hash, e);
                    } else {
                        omitted += 1;
                    }
                },
                Err(ref e) if e.kind() == ErrorKind::IoError(io::ErrorKind::UnexpectedEof) => break,
                Err(e) => return Err(e)
            }
        }
        println!("{} new entries", entries.len() - entries_len);
        if omitted > 0 {
            println!("{} entries are omitted as invalid", omitted);
        }
    }

    if entries.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "No valid state cache entries found"
        ));
    }

    println!(
        "Writing {} entries to file {}",
        entries.len(),
        config.output.file_name().and_then(OsStr::to_str).unwrap()
    );

    let header = DxvkStateCacheHeader {
        magic:      MAGIC_STRING,
        version:    config.version,
        entry_size: config.entry_size
    };

    let file = File::create(&config.output)?;
    let mut writer = BufWriter::new(file);
    wrtie_header(&mut writer, header)?;
    for (_, entry) in &entries {
        match config.edition {
            DxvkStateCacheEdition::Standard => write_entry(&mut writer, entry)?,
            DxvkStateCacheEdition::Legacy => write_entry_legacy(&mut writer, entry)?
        };
    }

    println!("Finished");

    Ok(())
}

fn read_header<R: Read>(reader: &mut BufReader<R>) -> Result<DxvkStateCacheHeader, Error> {
    Ok(DxvkStateCacheHeader {
        magic:      {
            let mut magic = [0; 4];
            reader.read_exact(&mut magic)?;
            magic
        },
        version:    reader.read_u32()?,
        entry_size: reader.read_u32()?
    })
}

fn read_entry<R: Read>(reader: &mut BufReader<R>) -> Result<DxvkStateCacheEntry, Error> {
    let header = DxvkStateCacheEntryHeader {
        stage_mask: reader.read_u8()?,
        entry_size: reader.read_u24()? as u32
    };
    let mut entry = DxvkStateCacheEntry::with_header(header);
    reader.read_exact(&mut entry.hash)?;
    reader.read_exact(&mut entry.data)?;

    Ok(entry)
}

fn read_entry_legacy<R: Read>(
    reader: &mut BufReader<R>,
    size: usize
) -> Result<DxvkStateCacheEntry, Error> {
    let mut entry = DxvkStateCacheEntry::with_length(size);
    reader.read_exact(&mut entry.data)?;
    reader.read_exact(&mut entry.hash)?;

    Ok(entry)
}

fn wrtie_header<W: Write>(
    writer: &mut BufWriter<W>,
    header: DxvkStateCacheHeader
) -> Result<(), Error> {
    writer.write_all(&MAGIC_STRING)?;
    writer.write_u32(header.version)?;
    writer.write_u32(header.entry_size as u32)?;

    Ok(())
}

fn write_entry<W: Write>(
    writer: &mut BufWriter<W>,
    entry: &DxvkStateCacheEntry
) -> Result<(), Error> {
    if let Some(h) = &entry.header {
        writer.write_u8(h.stage_mask)?;
        writer.write_u24(h.entry_size)?;
    }
    writer.write_all(&entry.hash)?;
    writer.write_all(&entry.data)?;

    Ok(())
}

fn write_entry_legacy<W: Write>(
    writer: &mut BufWriter<W>,
    entry: &DxvkStateCacheEntry
) -> Result<(), Error> {
    writer.write_all(&entry.data)?;
    writer.write_all(&entry.hash)?;

    Ok(())
}
