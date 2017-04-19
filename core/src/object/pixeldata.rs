//! Module for the pixel data trait and implementations.
//!
//! In order to facilitate typical pixel data manipulation, this crate
//! provides a common interface for retrieving that content as an image
//! or a multi-dimensional array.

use std::fmt;
use error::{Error, Result};

/** Implemented by DICOM pixel data blocks retrieved from objects.
 *
 * Pixel data elements typically represent 2D images. This trait provides
 * access to Pixel Data (7FE0,0010), Overlay Data (60xx,3000), or Waveform Data (5400,1010)
 * in a similar fashion to a two-dimensional array.
 * `PV` is a type used to represent a pixel.
 */
pub trait PixelData<PV> {
    // TODO this API is a work in progress.

    /// Get the number of rows (height) of the slice.
    fn rows(&self) -> u32;

    /// Get the number of columns (width) of the slice.
    fn columns(&self) -> u32;

    /// Get the number of bits used to represent each pixel.
    fn bits_per_pixel(&self) -> u32;

    /// Retrieve the number of samples (channels) per pixel.
    fn samples_per_pixel(&self) -> u16;

    /// Obtain the pixel value in the given position.
    /// Can return PixelDataOutOfBounds error when the given coordinates
    /// are out of the slice's boundaries.
    fn pixel_at(&self, width: u32, height: u32) -> Result<PV>;

    /// Obtain a mutable reference to the pixel value in the given position.
    /// Can return PixelDataOutOfBounds error when the given coordinates
    /// are out of the slice's boundaries.
    fn pixel_at_mut(&mut self, width: u32, height: u32) -> Result<&mut PV>;
}

/// A DICOM slice that is completely stored in memory.
/// Pixels are stored in row-major order with no padding.
pub struct InMemoryPixelData<P> {
    data: Vec<P>,
    rows: u32,
    cols: u32,
    bpp: u32,
    samples: u16,
}

impl<P: fmt::Debug> fmt::Debug for InMemoryPixelData<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "InMemoryPixelData[data={:?}, rows={}, cols={}, bbp={}, samples={}]",
               &self.data,
               self.rows,
               self.cols,
               self.bpp,
               self.samples)
    }
}

impl<P> InMemoryPixelData<P> {
    fn check_bounds(&self, w: u32, h: u32) -> Result<()> {
        if w >= self.cols || h >= self.rows {
            Err(Error::PixelDataOutOfBounds)
        } else {
            Ok(())
        }
    }

    /// Fetch the internal data vector, destroying the slice in the process.
    pub fn to_vector(self) -> Vec<P> {
        self.data
    }
}

impl<P> PixelData<P> for InMemoryPixelData<P>
    where P: Clone
{
    fn rows(&self) -> u32 {
        self.rows
    }

    fn columns(&self) -> u32 {
        self.cols
    }

    fn bits_per_pixel(&self) -> u32 {
        self.bpp
    }

    fn samples_per_pixel(&self) -> u16 {
        self.samples
    }

    fn pixel_at(&self, w: u32, h: u32) -> Result<P> {
        self.check_bounds(w, h).map(move |_| {
            let i = (h * self.cols + w) as usize;
            self.data[i].clone()
        })
    }

    fn pixel_at_mut(&mut self, w: u32, h: u32) -> Result<&mut P> {
        self.check_bounds(w, h).map(move |_| {
            let i = (h * self.cols + w) as usize;
            &mut self.data[i]
        })
    }
}
