//! Module for the pixel data trait and implementations.
//!
//! In order to facilitate typical pixel data manipulation, this crate
//! provides a common interface for retrieving that content as an image
//! or a multi-dimensional array.
use std::marker::PhantomData;
use snafu::{Backtrace, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
    PixelIndexOutOfBounds {
        backtrace: Backtrace,
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/** Implemented by DICOM pixel data blocks retrieved from objects.
 *
 * Pixel data elements typically represent 2D images. This trait provides
 * access to Pixel Data (7FE0,0010), Overlay Data (60xx,3000), or Waveform Data (5400,1010)
 * in a similar fashion to a two-dimensional array.
 */
pub trait PixelData {
    /// The representation of an individual pixel.
    type Pixel;

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
    fn pixel_at(&self, width: u32, height: u32) -> Result<Self::Pixel>;
}

pub trait PixelDataMut: PixelData {
    /// Obtain a mutable reference to the pixel value in the given position.
    /// Can return PixelDataOutOfBounds error when the given coordinates
    /// are out of the slice's boundaries.
    fn pixel_at_mut(&mut self, width: u32, height: u32) -> Result<&mut Self::Pixel>;
}

/// A DICOM slice that is completely stored in memory, which may be
/// owned by this  and owned by a local
/// vector. Pixels are stored in row-major order with no padding.
#[derive(Debug, Clone, PartialEq)]
pub struct InMemoryPixelData<C, P> {
    phantom: PhantomData<P>,
    data: C,
    rows: u32,
    cols: u32,
    bpp: u32,
    samples: u16,
}

impl<C, P> InMemoryPixelData<C, P> {
    fn check_bounds(&self, w: u32, h: u32) -> Result<()> {
        if w >= self.cols || h >= self.rows {
            PixelIndexOutOfBounds.fail()
        } else {
            Ok(())
        }
    }

    /// Fetch the internal data container, destroying the pixel data structure in the process.
    pub fn into_raw_data(self) -> C {
        self.data
    }

    /// Obtain a reference to the internal data container.
    pub fn raw_data(&self) -> &C {
        &self.data
    }

    /// Obtain a reference to the internal data container.
    pub fn raw_data_mut(&mut self) -> &mut C {
        &mut self.data
    }
}

impl<C, P> PixelData for InMemoryPixelData<C, P>
where
    P: Clone,
    C: std::ops::Deref<Target = [P]>,
{
    type Pixel = P;

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
}

impl<C, P> PixelDataMut for InMemoryPixelData<C, P>
where
    P: Clone,
    C: std::ops::DerefMut<Target = [P]>,
{
    fn pixel_at_mut(&mut self, w: u32, h: u32) -> Result<&mut P> {
        self.check_bounds(w, h).map(move |_| {
            let i = (h * self.cols + w) as usize;
            &mut self.data[i]
        })
    }
}
