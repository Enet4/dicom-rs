use std::io;
use std::io::{Read, Seek, SeekFrom};

/** A private type trait for the ability to efficiently implement stream skipping.
 */
pub trait ForwardSeek {
    fn skip(&mut self, n: u64) -> io::Result<u64>;
}

impl<S: ?Sized> ForwardSeek for S
where
    S: Seek,
{
    fn skip(&mut self, n: u64) -> io::Result<u64> {
        let curr_pos = self.stream_position()?;
        let new_pos = self.seek(SeekFrom::Current(n as i64))?;
        Ok(new_pos - curr_pos)
    }
}

/// A trait that combines for `Read` and `Seek`.
pub trait ReadSeek: Read + Seek {}
impl<T: ?Sized> ReadSeek for T where T: Read + Seek {}
