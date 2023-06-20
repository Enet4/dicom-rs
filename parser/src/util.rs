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

pub trait ReadSeek: Read + Seek {}
impl<T: ?Sized> ReadSeek for T where T: Read + Seek {}

/// Obtain an iterator of `n` void elements.
/// Useful for doing something N times as efficiently as possible.
pub fn n_times(n: usize) -> VoidRepeatN {
    VoidRepeatN { i: n }
}

pub struct VoidRepeatN {
    i: usize,
}

impl Iterator for VoidRepeatN {
    type Item = ();

    fn next(&mut self) -> Option<()> {
        match self.i {
            0 => None,
            _ => {
                self.i -= 1;
                Some(())
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.i, Some(self.i))
    }
}

impl ExactSizeIterator for VoidRepeatN {
    fn len(&self) -> usize {
        self.i
    }
}

#[cfg(test)]
mod tests {
    use super::n_times;

    #[test]
    fn void_repeat_n() {
        let it = n_times(5);
        assert_eq!(it.len(), 5);
        let mut k = 0;
        for v in it {
            assert_eq!(v, ());
            k += 1;
        }
        assert_eq!(k, 5);
        let mut it = n_times(0);
        assert_eq!(it.len(), 0);
        assert_eq!(it.next(), None);
    }
}
