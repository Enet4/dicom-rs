//! Floating point number conversion implementation for `PrimitiveValue`

use super::*;
use crate::value::ConvertValueError;

impl PrimitiveValue {
    /// Retrieve one single-precision floating point from this value.
    ///
    /// If the value is already represented as a number,
    /// it is returned after a conversion to [`f32`].
    /// An error is returned if the number cannot be represented
    /// by the given number type.
    /// If the value is a string or sequence of strings,
    /// the first string is parsed to obtain a number,
    /// potentially failing if the string does not represent a valid number.
    /// The string is stripped of leading/trailing whitespace before parsing.
    /// If the value is a sequence of [`U8`] bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::F32(smallvec![
    ///         1.5, 2., 5.,
    ///     ])
    ///     .to_float32().ok(),
    ///     Some(1.5_f32),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("-6.75 ").to_float32().ok(),
    ///     Some(-6.75),
    /// );
    /// ```
    pub fn to_float32(&self) -> Result<f32, ConvertValueError> {
        match self {
            PrimitiveValue::Str(s) => s
                .trim_matches(whitespace_or_null)
                .parse()
                .context(ParseFloatSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Strs(s) if !s.is_empty() => s[0]
                .trim_matches(whitespace_or_null)
                .parse()
                .context(ParseFloatSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::U8(bytes) if !bytes.is_empty() => {
                NumCast::from(bytes[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: bytes[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::U16(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::I16(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::U32(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::I32(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::U64(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::I64(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::F32(s) if !s.is_empty() => Ok(s[0]),
            PrimitiveValue::F64(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            _ => Err(ConvertValueError {
                requested: "float32",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a sequence of single-precision floating point numbers
    /// from this value.
    ///
    /// If the value is already represented as numbers,
    /// they are returned after a conversion to [`f32`].
    /// An error is returned if any of the numbers cannot be represented
    /// by an [`f32`].
    /// If the value is a string or sequence of strings,
    /// the strings are parsed to obtain a number,
    /// potentially failing if the string does not represent a valid number.
    /// The string is stripped of leading/trailing whitespace before parsing.
    /// If the value is a sequence of [`U8`] bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::F32(smallvec![
    ///         1.5, 2., 5.,
    ///     ])
    ///     .to_multi_float32().ok(),
    ///     Some(vec![1.5_f32, 2., 5.]),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("-6.75 ").to_multi_float32().ok(),
    ///     Some(vec![-6.75]),
    /// );
    /// ```
    pub fn to_multi_float32(&self) -> Result<Vec<f32>, ConvertValueError> {
        match self {
            PrimitiveValue::Empty => Ok(Vec::new()),
            PrimitiveValue::Str(s) => {
                let out = s
                    .trim_matches(whitespace_or_null)
                    .parse()
                    .context(ParseFloatSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })?;
                Ok(vec![out])
            }
            PrimitiveValue::Strs(s) => s
                .iter()
                .map(|v| {
                    v.trim_matches(whitespace_or_null)
                        .parse()
                        .context(ParseFloatSnafu)
                        .map_err(|err| ConvertValueError {
                            requested: "float32",
                            original: self.value_type(),
                            cause: Some(Box::from(err)),
                        })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U8(bytes) => bytes
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U16(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I16(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U32(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I32(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U64(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I64(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::F32(s) => Ok(s[..].to_owned()),
            PrimitiveValue::F64(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            _ => Err(ConvertValueError {
                requested: "float32",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve one double-precision floating point from this value.
    ///
    /// If the value is already represented as a number,
    /// it is returned after a conversion to [`f64`].
    /// An error is returned if the number cannot be represented
    /// by the given number type.
    /// If the value is a string or sequence of strings,
    /// the first string is parsed to obtain a number,
    /// potentially failing if the string does not represent a valid number.
    /// The string is stripped of leading/trailing whitespace before parsing.
    /// If the value is a sequence of [`U8`] bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::F64(smallvec![
    ///         1.5, 2., 5.,
    ///     ])
    ///     .to_float64().ok(),
    ///     Some(1.5_f64),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("-6.75 ").to_float64().ok(),
    ///     Some(-6.75),
    /// );
    /// ```
    pub fn to_float64(&self) -> Result<f64, ConvertValueError> {
        match self {
            PrimitiveValue::Str(s) => s
                .trim_matches(whitespace_or_null)
                .parse()
                .context(ParseFloatSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Strs(s) if !s.is_empty() => s[0]
                .trim_matches(whitespace_or_null)
                .parse()
                .context(ParseFloatSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::U8(bytes) if !bytes.is_empty() => {
                NumCast::from(bytes[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: bytes[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::U16(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::I16(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::U32(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::I32(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::U64(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::I64(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::F32(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvertSnafu {
                            value: s[0].to_string(),
                        }
                        .build()
                        .into(),
                    ),
                })
            }
            PrimitiveValue::F64(s) if !s.is_empty() => Ok(s[0]),
            _ => Err(ConvertValueError {
                requested: "float64",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a sequence of double-precision floating point numbers
    /// from this value.
    ///
    /// If the value is already represented as numbers,
    /// they are returned after a conversion to [`f64`].
    /// An error is returned if any of the numbers cannot be represented
    /// by an [`f64`].
    /// If the value is a string or sequence of strings,
    /// the strings are parsed to obtain a number,
    /// potentially failing if the string does not represent a valid number.
    /// The string is stripped of leading/trailing whitespace before parsing.
    /// If the value is a sequence of [`U8`] bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::F64(smallvec![
    ///         1.5, 2., 5.,
    ///     ])
    ///     .to_multi_float64().ok(),
    ///     Some(vec![1.5_f64, 2., 5.]),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("-6.75 ").to_multi_float64().ok(),
    ///     Some(vec![-6.75]),
    /// );
    /// ```
    pub fn to_multi_float64(&self) -> Result<Vec<f64>, ConvertValueError> {
        match self {
            PrimitiveValue::Str(s) => {
                let out = s
                    .trim_matches(whitespace_or_null)
                    .parse()
                    .context(ParseFloatSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })?;
                Ok(vec![out])
            }
            PrimitiveValue::Strs(s) => s
                .iter()
                .map(|v| {
                    v.trim_matches(whitespace_or_null)
                        .parse()
                        .context(ParseFloatSnafu)
                        .map_err(|err| ConvertValueError {
                            requested: "float64",
                            original: self.value_type(),
                            cause: Some(Box::from(err)),
                        })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U8(bytes) => bytes
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U16(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I16(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U32(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I32(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U64(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I64(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::F32(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvertSnafu {
                                value: v.to_string(),
                            }
                            .build()
                            .into(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::F64(s) => Ok(s[..].to_owned()),
            _ => Err(ConvertValueError {
                requested: "float32",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Extend a value of numbers by appending
    /// 32-bit floating point numbers to an existing value.
    ///
    /// The value may be empty
    /// or already contain numeric or textual values.
    ///
    /// If the current value is textual,
    /// the numbers provided are converted to text.
    /// For the case of numeric values,
    /// the given numbers are _converted to the current number type
    /// through casting_,
    /// meaning that loss of precision may occur.
    /// If this is undesirable,
    /// read the current value and replace it manually.
    ///
    /// An error is returned
    /// if the current value is not compatible with the insertion of numbers,
    /// such as [`Tag`] or [`Date`].
    ///
    /// [`Date`]: Self::Date
    ///
    /// # Example
    ///
    /// ```
    /// use dicom_core::dicom_value;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut value = dicom_value!(F32, [1., 2.]);
    /// value.extend_f32([5.])?;
    /// assert_eq!(value.to_multi_float32()?, vec![1., 2., 5.]);
    ///
    /// let mut value = dicom_value!(Strs, ["1.25"]);
    /// value.extend_f32([0.5])?;
    /// assert_eq!(value.to_string(), "1.25\\0.5");
    /// # Ok(())
    /// # }
    /// ```
    pub fn extend_f32(
        &mut self,
        numbers: impl IntoIterator<Item = f32>,
    ) -> Result<(), ModifyValueError> {
        match self {
            PrimitiveValue::Empty => {
                *self = PrimitiveValue::F32(numbers.into_iter().collect());
                Ok(())
            }
            PrimitiveValue::Strs(elements) => {
                elements.extend(numbers.into_iter().map(|n| n.to_string()));
                Ok(())
            }
            PrimitiveValue::Str(s) => {
                // for lack of better ways to move the string out from the mutable borrow,
                // we create a copy for now
                let s = s.clone();
                *self = PrimitiveValue::Strs(
                    std::iter::once(s)
                        .chain(numbers.into_iter().map(|n| n.to_string()))
                        .collect(),
                );
                Ok(())
            }
            PrimitiveValue::F32(elements) => {
                elements.extend(numbers);
                Ok(())
            }
            PrimitiveValue::U8(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u8));
                Ok(())
            }
            PrimitiveValue::I16(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as i16));
                Ok(())
            }
            PrimitiveValue::U16(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u16));
                Ok(())
            }
            PrimitiveValue::I32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as i32));
                Ok(())
            }
            PrimitiveValue::I64(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as i64));
                Ok(())
            }
            PrimitiveValue::U32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u32));
                Ok(())
            }
            PrimitiveValue::U64(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u64));
                Ok(())
            }
            PrimitiveValue::F64(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as f64));
                Ok(())
            }
            PrimitiveValue::Tags(_)
            | PrimitiveValue::Date(_)
            | PrimitiveValue::DateTime(_)
            | PrimitiveValue::Time(_) => IncompatibleNumberTypeSnafu {
                original: self.value_type(),
            }
            .fail(),
        }
    }

    /// Extend a value of numbers by appending
    /// 64-bit floating point numbers to an existing value.
    ///
    /// The value may be empty
    /// or already contain numeric or textual values.
    ///
    /// If the current value is textual,
    /// the numbers provided are converted to text.
    /// For the case of numeric values,
    /// the given numbers are _converted to the current number type
    /// through casting_,
    /// meaning that loss of precision may occur.
    /// If this is undesirable,
    /// read the current value and replace it manually.
    ///
    /// An error is returned
    /// if the current value is not compatible with the insertion of numbers,
    /// such as [`Tag`] or [`Date`].
    ///
    /// [`Date`]: Self::Date
    ///
    /// # Example
    ///
    /// ```
    /// use dicom_core::dicom_value;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut value = dicom_value!(F64, [1., 2.]);
    /// value.extend_f64([5.])?;
    /// assert_eq!(value.to_multi_float64()?, vec![1., 2., 5.]);
    ///
    /// let mut value = dicom_value!(Strs, ["1.25"]);
    /// value.extend_f64([0.5])?;
    /// assert_eq!(value.to_string(), "1.25\\0.5");
    /// # Ok(())
    /// # }
    /// ```
    pub fn extend_f64(
        &mut self,
        numbers: impl IntoIterator<Item = f64>,
    ) -> Result<(), ModifyValueError> {
        match self {
            PrimitiveValue::Empty => {
                *self = PrimitiveValue::F64(numbers.into_iter().collect());
                Ok(())
            }
            PrimitiveValue::Strs(elements) => {
                elements.extend(numbers.into_iter().map(|n| n.to_string()));
                Ok(())
            }
            PrimitiveValue::Str(s) => {
                // for lack of better ways to move the string out from the mutable borrow,
                // we create a copy for now
                let s = s.clone();
                *self = PrimitiveValue::Strs(
                    std::iter::once(s)
                        .chain(numbers.into_iter().map(|n| n.to_string()))
                        .collect(),
                );
                Ok(())
            }
            PrimitiveValue::F64(elements) => {
                elements.extend(numbers);
                Ok(())
            }
            PrimitiveValue::U8(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u8));
                Ok(())
            }
            PrimitiveValue::I16(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as i16));
                Ok(())
            }
            PrimitiveValue::U16(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u16));
                Ok(())
            }
            PrimitiveValue::I32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as i32));
                Ok(())
            }
            PrimitiveValue::I64(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as i64));
                Ok(())
            }
            PrimitiveValue::U32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u32));
                Ok(())
            }
            PrimitiveValue::U64(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u64));
                Ok(())
            }
            PrimitiveValue::F32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as f32));
                Ok(())
            }
            PrimitiveValue::Tags(_)
            | PrimitiveValue::Date(_)
            | PrimitiveValue::DateTime(_)
            | PrimitiveValue::Time(_) => Err(IncompatibleNumberTypeSnafu {
                original: self.value_type(),
            }
            .build()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dicom_value;
    use crate::value::{ConvertValueError, InvalidValueReadError, PrimitiveValue, ValueType};

    #[test]
    fn primitive_value_to_float() {
        // DS conversion to f32
        assert_eq!(dicom_value!(Str, "-73.4 ").to_float32().ok(), Some(-73.4));

        // DS conversion with leading whitespaces
        assert_eq!(dicom_value!(Str, " -73.4 ").to_float32().ok(), Some(-73.4));

        // DS conversion with leading whitespaces
        assert_eq!(dicom_value!(Str, " -73.4 ").to_float64().ok(), Some(-73.4));

        // DS conversion with exponential
        assert_eq!(dicom_value!(Str, "1e1").to_float32().ok(), Some(10.0));
    }

    #[test]
    fn primitive_value_to_multi_floats() {
        assert_eq!(PrimitiveValue::Empty.to_multi_float32().ok(), Some(vec![]));

        let test_value = dicom_value!(U16, [1, 2, 3, 4]);

        assert_eq!(
            test_value.to_multi_float32().ok(),
            Some(vec![1., 2., 3., 4.]),
        );
        assert_eq!(
            test_value.to_multi_float64().ok(),
            Some(vec![1., 2., 3., 4.]),
        );

        // admits a number as text, trailing space too
        assert_eq!(
            dicom_value!(Strs, ["7.25", "-12.5 "])
                .to_multi_float64()
                .ok(),
            Some(vec![7.25, -12.5]),
        );

        // does not admit strings which are not numbers
        assert!(matches!(
            dicom_value!(Strs, ["Smith^John"]).to_multi_float64(),
            Err(ConvertValueError {
                requested: _,
                original: ValueType::Strs,
                // would try to parse as a float and fail
                cause: Some(cause),
            }) if matches!(&*cause, InvalidValueReadError::ParseFloat { .. })
        ));
    }
}
