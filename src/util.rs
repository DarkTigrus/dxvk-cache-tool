use std::io::{self, BufReader, Read};

impl<R: Read> ReadEx for BufReader<R> {}
pub trait ReadEx: Read {
    fn read_u32(&mut self) -> io::Result<u32> {
        let mut buf = [0; 4];
        match self.read_exact(&mut buf) {
            Ok(_) => Ok(u32::from_le_bytes(buf)),
            Err(e) => Err(e)
        }
    }
}
