//! Look-up table (LUT) implementation and transformation functions.
//!
//! This module contains the [`Lut`] data type,
//! designed to turn pixel data native sample values
//! (as encoded in the _Pixel Data_ attribute)
//! into displayable or otherwise more meaningful values.
//!
//! The type also provides easy-to-use constructor functions
//! for common DICOM sample value transformations.

use num_traits::{NumCast, ToPrimitive};
#[cfg(feature = "rayon")]
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use snafu::{OptionExt, Snafu};

use crate::{Rescale, WindowLevelTransform};

/// The LUT could not be created:
/// entry #{index} was mapped to {y_value},
/// which could not be cast to the target type.
#[derive(Debug, PartialEq, Snafu)]
pub struct CreateLutError {
    index: usize,
    y_value: f64,
}

impl CreateLutError {
    /// Get the original index in the LUT.
    pub fn index(&self) -> usize {
        self.index
    }
    /// Get the value which could not be converted to the target type.
    pub fn y_value(&self) -> f64 {
        self.y_value
    }
}

/// A look up table for pixel data sample value transformations.
///
/// # Example
///
/// The LUT can be populated with common transformations
/// via the functions [`new_rescale`](Lut::new_rescale),
/// [`new_window`](Lut::new_window),
/// and [`new_rescale_and_window`](Lut::new_rescale_and_window).
/// The function [`new_with_fn`](Lut::new_with_fn)
/// can be used to create a LUT with a custom function.
///
/// ```
/// # use dicom_pixeldata::{
/// #     CreateLutError, Lut, Rescale, VoiLutFunction,
/// #     WindowLevel, WindowLevelTransform,
/// # };
/// let bits_stored = 8;
/// let lut = Lut::new_rescale_and_window(
///     bits_stored,
///     false,
///     Rescale::new(1., -1024.),
///     WindowLevelTransform::new(
///         VoiLutFunction::Linear,
///         WindowLevel {
///             width: 300.,
///             center: 50.
///         }
///     ),
/// )?;
///
/// let val: u8 = lut.get(100_u8);
/// # Result::<(), CreateLutError>::Ok(())
/// ```
#[derive(Debug)]
pub struct Lut<T> {
    /// the table which maps an index to a transformed value,
    /// of size 2 to the power of `bits_stored`
    table: Vec<T>,
    /// whether the input sample values are signed (Pixel Representation = 1)
    signed: bool,
}

impl<T: 'static> Lut<T>
where
    T: NumCast,
    T: Copy,
    T: Send + Sync,
{
    /// Create a new LUT with the given characteristics
    /// and populates it with the outputs of the provided function.
    /// The function may be called concurrently.
    ///
    /// - `bits_stored`:
    ///   the number of bits effectively used to represent the sample values
    ///   (the _Bits Stored_ DICOM attribute)
    /// - `signed`:
    ///   whether the input sample values are expected to be signed
    ///   (_Pixel Representation_ = 1)
    /// - `f`: the mapping function
    ///
    /// # Panics
    ///
    /// Panics if `bits_stored` is 0 or too large.
    pub fn new_with_fn(
        bits_stored: u16,
        signed: bool,
        f: impl Fn(f64) -> f64 + Sync,
    ) -> Result<Self, CreateLutError> {
        assert!(bits_stored != 0 && bits_stored <= 32);
        let size = (1 << bits_stored as u32) as usize;
        debug_assert!(size.is_power_of_two());

        #[cfg(feature = "rayon")]
        let iter = (0..size).into_par_iter();
        #[cfg(not(feature = "rayon"))]
        let iter = (0..size).into_iter();

        let table: Result<Vec<_>, _> = iter
            .map(|i| {
                // account for signedness to determine input pixel value
                let x = if signed && i >= size / 2 {
                    i as f64 - size as f64
                } else {
                    i as f64
                };
                let value = f(x);
                T::from(value).context(CreateLutSnafu {
                    index: i,
                    y_value: value,
                })
            })
            .collect();
        Ok(Self {
            table: table?,
            signed,
        })
    }

    /// Create a new LUT containing only the modality rescale transformation.
    ///
    /// - `bits_stored`:
    ///   the number of bits effectively used to represent the sample values
    ///   (the _Bits Stored_ DICOM attribute)
    /// - `signed`:
    ///   whether the input sample values are expected to be signed
    ///   (_Pixel Representation_ = 1)
    /// - `rescale`: the rescale parameters
    ///
    /// # Panics
    ///
    /// Panics if `bits_stored` is 0 or too large.
    pub fn new_rescale(
        bits_stored: u16,
        signed: bool,
        rescale: Rescale,
    ) -> Result<Self, CreateLutError> {
        Self::new_with_fn(bits_stored, signed, |v| rescale.apply(v))
    }

    /// Create a new LUT containing the given modality rescale transformation
    /// and a min-max normalization
    /// which satisfies the raw samples given.
    /// The sample type `I` is expected to be either `u8` or `u16`,
    /// even if the sample is meant to be interpreted as signed.
    ///
    /// - `bits_stored`:
    ///   the number of bits effectively used to represent the sample values
    ///   (the _Bits Stored_ DICOM attribute)
    /// - `signed`:
    ///   whether the input sample values are expected to be signed
    ///   (_Pixel Representation_ = 1)
    /// - `rescale`: the rescale parameters
    /// - `samples`: the raw pixel data samples expected to be fed to the LUT
    ///
    /// # Panics
    ///
    /// Panics if `bits_stored` is 0 or too large.
    pub(crate) fn new_rescale_and_normalize<I>(
        bits_stored: u16,
        signed: bool,
        rescale: Rescale,
        samples: I,
    ) -> Result<Self, CreateLutError>
    where
        I: IntoIterator,
        I: Clone,
        I::IntoIter: Clone,
        I::Item: Clone,
        I::Item: NumCast,
        I::Item: ToPrimitive,
    {
        let samples_f64 = samples.into_iter().filter_map(|v| v.to_f64());
        let min: f64 = samples_f64.clone().fold(f64::MAX, |a, b| a.min(b));
        let max: f64 = samples_f64.fold(f64::MIN, |a, b| a.max(b));

        // convert to f64 and swap if negative slope
        let (min, max) = if rescale.slope < 0. {
            (max, min)
        } else {
            (min, max)
        };

        let max = max * rescale.slope + rescale.intercept;
        let min = min * rescale.slope + rescale.intercept;

        // create a linear window level transform
        let voi = WindowLevelTransform::linear(crate::WindowLevel {
            width: max - min + 1.,
            center: (min + max) / 2.,
        });

        Self::new_rescale_and_window(bits_stored, signed, rescale, voi)
    }

    /// Create a new LUT containing the modality rescale transformation
    /// and the VOI transformation defined by a window level.
    ///
    /// The amplitude of the output values
    /// goes from 0 to `2^n - 1`, where `n` is the power of two
    /// which follows `bits_stored` (or itself if it is a power of two).
    /// For instance, if `bits_stored` is 12, the output values
    /// will go from 0 to 65535.
    ///
    /// - `bits_stored`:
    ///   the number of bits effectively used to represent the sample values
    ///   (the _Bits Stored_ DICOM attribute)
    /// - `signed`:
    ///   whether the input sample values are expected to be signed
    ///   (_Pixel Representation_ = 1)
    /// - `rescale`: the rescale parameters
    /// - `voi`: the value of interest (VOI) function and parameters
    ///
    /// # Panics
    ///
    /// Panics if `bits_stored` is 0 or too large.
    pub fn new_rescale_and_window(
        bits_stored: u16,
        signed: bool,
        rescale: Rescale,
        voi: WindowLevelTransform,
    ) -> Result<Self, CreateLutError> {
        let bits_allocated = (bits_stored as usize).next_power_of_two();
        let y_max = ((1 << bits_allocated) - 1) as f64;
        Self::new_with_fn(bits_stored, signed, |v| {
            let x = v as f64;
            let v = rescale.apply(x);
            voi.apply(v, y_max)
        })
    }

    /// Create a new LUT containing
    /// a VOI transformation defined by a window level.
    ///
    /// The amplitude of the output values
    /// goes from 0 to `2^n - 1`, where `n` is the power of two
    /// which follows `bits_stored` (or itself if it is a power of two).
    /// For instance, if `bits_stored` is 12, the output values
    /// will go from 0 to 65535.
    ///
    /// - `bits_stored`:
    ///   the number of bits effectively used to represent the sample values
    ///   (the _Bits Stored_ DICOM attribute)
    /// - `signed`:
    ///   whether the input sample values are expected to be signed
    ///   (_Pixel Representation_ = 1)
    /// - `voi`: the value of interest (VOI) function and parameters
    ///
    /// # Panics
    ///
    /// Panics if `bits_stored` is 0 or too large.
    pub fn new_window(
        bits_stored: u16,
        signed: bool,
        voi: WindowLevelTransform,
    ) -> Result<Self, CreateLutError> {
        let bits_allocated = (bits_stored as usize).next_power_of_two();
        let y_max = ((1 << bits_allocated) - 1) as f64;
        Self::new_with_fn(bits_stored, signed, |v| voi.apply(v as f64, y_max))
    }

    /// Apply the transformation to a single pixel sample value.
    ///
    /// Although the input is expected to be one of `u8`, `u16`, or `u32`,
    /// this method works for signed sample values as well,
    /// with the bits reinterpreted as their unsigned counterpart.
    ///
    /// # Panics
    ///
    /// Panics if `sample_value` is larger or equal to `2^bits_stored`.
    pub fn get<I: 'static>(&self, sample_value: I) -> T
    where
        I: Copy,
        I: Into<u32>,
    {
        let val = sample_value.into();
        let index = if self.signed {
            // adjust for signedness by masking out the extra sign bits
            let mask = self.table.len() - 1;
            val as usize & mask
        } else {
            val as usize
        };
        assert!((index as usize) < self.table.len());

        self.table[index as usize]
    }

    /// Adapts an iterator of pixel data sample values
    /// to an iterator of transformed values.
    pub fn map_iter<'a, I: 'static>(
        &'a self,
        iter: impl IntoIterator<Item = I> + 'a,
    ) -> impl Iterator<Item = T> + 'a
    where
        I: Copy,
        I: Into<u32>,
    {
        iter.into_iter().map(move |i| self.get(i))
    }

    /// Adapts a parallel iterator of pixel data sample values
    /// to a parallel iterator of transformed values.
    #[cfg(feature = "rayon")]
    pub fn map_par_iter<'a, I: 'static>(
        &'a self,
        iter: impl ParallelIterator<Item = I> + 'a,
    ) -> impl ParallelIterator<Item = T> + 'a
    where
        I: Copy,
        I: Into<u32>,
    {
        iter.map(move |i| self.get(i))
    }
}

#[cfg(test)]
mod tests {
    use crate::{VoiLutFunction, WindowLevel};

    use super::*;

    /// Applying a common rescale function to a value
    /// gives the expected output.
    #[test]
    fn modality_lut_baseline_2() {
        let rescale = Rescale::new(2., -1024.);

        assert_eq!(rescale.apply(0.), -1024.);
        assert_eq!(rescale.apply(1.), -1022.);
        assert_eq!(rescale.apply(2.), -1020.);
        assert_eq!(rescale.apply(1024.), 1024.);
    }

    #[test]
    fn lut_signed_numbers() {
        // 10-bit precision input, signed output
        let lut: Lut<i16> = Lut::new_rescale(10, true, Rescale::new(2., -1024.)).unwrap();

        assert_eq!(lut.get(0 as u16), -1024);
        assert_eq!(lut.get(1 as u16), -1022);
        assert_eq!(lut.get(-1_i16 as u16), -1026);
        assert_eq!(lut.get(-2_i16 as u16), -1028);
        assert_eq!(lut.get(500 as u16), -24);
    }

    #[test]
    fn lut_rescale_and_window_16bit() {
        let bits_stored = 16;
        let lut = Lut::new_rescale_and_window(
            bits_stored,
            false,
            Rescale::new(1., -1024.),
            WindowLevelTransform::new(
                VoiLutFunction::Linear,
                WindowLevel {
                    width: 300.,
                    center: 50.,
                },
            ),
        )
        .unwrap();

        // < 0
        let val: u16 = lut.get(824_u16);
        assert_eq!(val, 0);

        // > 200
        let val: u16 = lut.get(1224_u16);
        assert_eq!(val, 65535);

        // around the middle

        let val: u16 = lut.get(1024_u16 + 50);
        let expected_range = 32_500..=33_000;
        assert!(
            expected_range.contains(&val),
            "outcome was {}, expected to be in {:?}",
            val,
            expected_range,
        );
    }

    #[test]
    fn lut_rescale_and_normalize_16bit() {
        let bits_stored = 16;
        let lut: Lut<u16> = Lut::new_rescale_and_normalize(
            bits_stored,
            false,
            Rescale::new(1., -1024.),
            [0_u16, 1, 2, 500, 23].iter().copied(),
        )
        .unwrap();

        // samples are between 0 and 500
        assert_eq!(lut.get(0_u16), 0);
        assert_eq!(lut.get(500_u16), 0xFFFF);

        let y = lut.get(1_u16);
        assert!(y > 0 && y < 0xFFFF);

        let y = lut.get(498_u16);
        assert!(y > 0 && y < 0xFFFF);
    }
}
