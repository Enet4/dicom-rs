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
    /// An error is returned if
    /// any of the numbers cannot be represented by an [`f32`].
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

    /// Retrieve one finite single-precision floating point from this value.
    ///
    /// If the value is already represented as a number,
    /// it is returned after a conversion to [`f32`].
    /// An error is returned if
    /// the number cannot be represented by the given number type
    /// or the number is not finite (e.g. NaN).
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
    /// assert_eq!(
    ///     PrimitiveValue::F32(smallvec![
    ///         1.5, 2., 5.,
    ///     ])
    ///     .to_finite_float32().ok(),
    ///     Some(1.5_f32),
    /// );
    ///
    /// assert!(
    ///     PrimitiveValue::from("NaN").to_finite_float32().is_err()
    /// );
    /// ```
    pub fn to_finite_float32(&self) -> Result<f32, ConvertValueError> {
        let v = self.to_float32().map_err(|mut e| {
            e.requested = "finite float32";
            e
        })?;
        if !v.is_finite() {
            return Err(ConvertValueError {
                original: self.value_type(),
                requested: "finite float32",
                cause: None,
            });
        }
        Ok(v)
    }

    /// Retrieve a sequence of single-precision floating point numbers
    /// from this value.
    ///
    /// If the value is already represented as numbers,
    /// they are returned after a conversion to [`f32`].
    /// An error is returned if
    /// any of the items cannot be represented by an [`f32`]
    /// or any of the numbers are not finite (e.g. NaN).
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
    ///     .to_multi_finite_float32().ok(),
    ///     Some(vec![1.5_f32, 2., 5.]),
    /// );
    ///
    /// assert!(
    ///     PrimitiveValue::Strs(smallvec!["-6.75".to_string(), "NaN".to_string()]).to_multi_finite_float32().is_err(),
    /// );
    /// ```
    pub fn to_multi_finite_float32(&self) -> Result<Vec<f32>, ConvertValueError> {
        let values = self.to_multi_float32().map_err(|mut e| {
            e.requested = "finite float32";
            e
        })?;
        if values.iter().any(|v| !v.is_finite()) {
            return Err(ConvertValueError {
                original: self.value_type(),
                requested: "finite float32",
                cause: None,
            });
        }
        Ok(values)
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

    /// Retrieve one finite double-precision floating point from this value.
    ///
    /// If the value is already represented as a number,
    /// it is returned after a conversion to [`f64`].
    /// An error is returned if
    /// the number cannot be represented by the given number type
    /// or the resulting floating point value is not finite (e.g. NaN).
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
    /// assert_eq!(
    ///     PrimitiveValue::F64(smallvec![
    ///         1.2222e8
    ///     ])
    ///     .to_finite_float64().ok(),
    ///     Some(1.2222e8_f64),
    /// );
    ///
    /// assert!(
    ///     PrimitiveValue::from("NaN").to_finite_float64().is_err(),
    /// );
    /// ```
    pub fn to_finite_float64(&self) -> Result<f64, ConvertValueError> {
        let v = self.to_float64().map_err(|mut e| {
            e.requested = "finite float64";
            e
        })?;
        if !v.is_finite() {
            return Err(ConvertValueError {
                original: self.value_type(),
                requested: "finite float64",
                cause: None,
            });
        }
        Ok(v)
    }

    /// Retrieve a sequence of finite double-precision floating point numbers
    /// from this value.
    ///
    /// If the value is already represented as numbers,
    /// they are returned after a conversion to [`f64`].
    /// An error is returned if
    /// any of the numbers cannot be represented by an [`f64`]
    /// or any of the numbers are not finite (e.g. NaN).
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
    ///     .to_multi_finite_float64().ok(),
    ///     Some(vec![1.5_f64, 2., 5.]),
    /// );
    ///
    /// assert!(
    ///     PrimitiveValue::Strs(smallvec!["-6.75".to_string(), "NaN".to_string()])
    ///         .to_multi_finite_float64()
    ///         .is_err()
    /// );
    /// ```
    pub fn to_multi_finite_float64(&self) -> Result<Vec<f64>, ConvertValueError> {
        let values = self.to_multi_float64().map_err(|mut e| {
            e.requested = "finite float64";
            e
        })?;
        if values.iter().any(|v| !v.is_finite()) {
            return Err(ConvertValueError {
                original: self.value_type(),
                requested: "finite float64",
                cause: None,
            });
        }
        Ok(values)
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

        // this method accepts non-finite numbers
        assert!(dicom_value!(Str, "NaN").to_float32().unwrap().is_nan());
        assert!(dicom_value!(Str, "NaN").to_float64().unwrap().is_nan());
        assert_eq!(dicom_value!(Str, "Infinity").to_float32().ok(), Some(std::f32::INFINITY));
        assert_eq!(dicom_value!(Str, "Infinity").to_float64().ok(), Some(std::f64::INFINITY));
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

    #[test]
    fn primitive_value_to_finite_float() {
        // DS conversion to f32 and f64
        assert_eq!(dicom_value!(Str, "-73.4 ").to_finite_float32().ok(), Some(-73.4));
        assert_eq!(dicom_value!(Str, "-73.4 ").to_finite_float64().ok(), Some(-73.4));

        // DS conversion with leading whitespaces
        assert_eq!(dicom_value!(Str, " -73.4 ").to_finite_float32().ok(), Some(-73.4));
        assert_eq!(dicom_value!(Str, " -73.4 ").to_finite_float64().ok(), Some(-73.4));

        // DS conversion with leading whitespaces
        assert_eq!(dicom_value!(Str, " -73.4 ").to_finite_float32().ok(), Some(-73.4));
        assert_eq!(dicom_value!(Str, " -73.4 ").to_finite_float64().ok(), Some(-73.4));

        // DS conversion with exponential
        assert_eq!(dicom_value!(Str, "1e1").to_finite_float32().ok(), Some(10.0));
        assert_eq!(dicom_value!(Str, "12e12").to_finite_float64().ok(), Some(12e12));

        // DS does not accept NaNs or Infinity
        assert!(matches!(dicom_value!(Str, "NaN").to_finite_float32(), Err(_)));
        assert!(matches!(dicom_value!(Str, "NaN").to_finite_float64(), Err(_)));
        assert!(matches!(dicom_value!(Str, "Infinity").to_finite_float32(), Err(_)));
        assert!(matches!(dicom_value!(Str, "Infinity").to_finite_float64(), Err(_)));
    }
}
