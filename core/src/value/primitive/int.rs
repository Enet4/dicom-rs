//! Integer conversion implementation for `PrimitiveValue`

use super::*;
use crate::value::ConvertValueError;

impl PrimitiveValue {
    /// Retrieve a single integer of type `T` from this value.
    ///
    /// If the value is already represented as an integer,
    /// it is returned after a conversion to the target type.
    /// An error is returned if the integer cannot be represented
    /// by the given integer type.
    /// If the value is a string or sequence of strings,
    /// the first string is parsed to obtain an integer,
    /// potentially failing if the string does not represent a valid integer.
    /// The string is stripped of leading/trailing whitespace before parsing.
    /// If the value is a sequence of U8 bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// Note that this method does not enable
    /// the conversion of floating point numbers to integers via truncation.
    /// If this is intentional,
    /// retrieve a float via [`to_float32`] or [`to_float64`] instead,
    /// then cast it to an integer.
    ///
    /// [`to_float32`]: Self::to_float32
    /// [`to_float64`]: Self::to_float64
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::I32(smallvec![
    ///         1, 2, 5,
    ///     ])
    ///     .to_int::<u32>().ok(),
    ///     Some(1_u32),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("505 ").to_int::<i32>().ok(),
    ///     Some(505),
    /// );
    /// ```
    pub fn to_int<T>(&self) -> Result<T, ConvertValueError>
    where
        T: NumCast,
        T: FromStr<Err = std::num::ParseIntError>,
    {
        match self {
            PrimitiveValue::Str(s) => s
                .trim_matches(whitespace_or_null)
                .parse()
                .context(ParseIntegerSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "integer",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Strs(s) if !s.is_empty() => s[0]
                .trim_matches(whitespace_or_null)
                .parse()
                .context(ParseIntegerSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "integer",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::U8(bytes) if !bytes.is_empty() => {
                T::from(bytes[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
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
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
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
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
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
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
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
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
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
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
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
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
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
                requested: "integer",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a sequence of integers of type `T` from this value.
    ///
    /// If the values is already represented as an integer,
    /// it is returned after a [`NumCast`] conversion to the target type.
    /// An error is returned if any of the integers cannot be represented
    /// by the given integer type.
    /// If the value is a string or sequence of strings,
    /// each string is parsed to obtain an integer,
    /// potentially failing if the string does not represent a valid integer.
    /// The string is stripped of leading/trailing whitespace before parsing.
    /// If the value is a sequence of [`U8`] bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// Note that this method does not enable
    /// the conversion of floating point numbers to integers via truncation.
    /// If this is intentional,
    /// retrieve a float via [`to_float32`] or
    /// [`to_float64`] instead, then cast it to
    /// an integer.
    ///
    /// [`U8`]: Self::U8
    /// [`to_float32`]: Self::to_float32
    /// [`to_float64`]: Self::to_float64
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use dicom_core::dicom_value;
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::I32(smallvec![
    ///         1, 2, 5,
    ///     ])
    ///     .to_multi_int::<u32>().ok(),
    ///     Some(vec![1_u32, 2, 5]),
    /// );
    ///
    /// assert_eq!(
    ///     dicom_value!(Strs, ["5050", "23 "]).to_multi_int::<i32>().ok(),
    ///     Some(vec![5050, 23]),
    /// );
    /// ```
    pub fn to_multi_int<T>(&self) -> Result<Vec<T>, ConvertValueError>
    where
        T: NumCast,
        T: FromStr<Err = std::num::ParseIntError>,
    {
        match self {
            PrimitiveValue::Empty => Ok(Vec::new()),
            PrimitiveValue::Str(s) => {
                let out = s
                    .trim_matches(whitespace_or_null)
                    .parse()
                    .context(ParseIntegerSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "integer",
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
                        .context(ParseIntegerSnafu)
                        .map_err(|err| ConvertValueError {
                            requested: "integer",
                            original: self.value_type(),
                            cause: Some(Box::from(err)),
                        })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U8(bytes) => bytes
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
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
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
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
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
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
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
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
            PrimitiveValue::I32(s) if !s.is_empty() => s
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
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
            PrimitiveValue::U64(s) if !s.is_empty() => s
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
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
            PrimitiveValue::I64(s) if !s.is_empty() => s
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
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
                requested: "integer",
                original: self.value_type(),
                cause: None,
            }),
        }
    }
    /// Extend a value of numbers by appending
    /// 16-bit unsigned integers to an existing value.
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
    /// if the current value is not compatible with the insertion of integers,
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
    /// let mut value = dicom_value!(U16, [1, 2]);
    /// value.extend_u16([5])?;
    /// assert_eq!(value.to_multi_int::<u16>()?, vec![1, 2, 5]);
    ///
    /// let mut value = dicom_value!(Strs, ["City"]);
    /// value.extend_u16([17])?;
    /// assert_eq!(value.to_string(), "City\\17");
    /// # Ok(())
    /// # }
    /// ```
    pub fn extend_u16(
        &mut self,
        numbers: impl IntoIterator<Item = u16>,
    ) -> Result<(), ModifyValueError> {
        match self {
            PrimitiveValue::Empty => {
                *self = PrimitiveValue::U16(numbers.into_iter().collect());
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
            PrimitiveValue::U16(elements) => {
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
            PrimitiveValue::U32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u32));
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
            PrimitiveValue::U64(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u64));
                Ok(())
            }
            PrimitiveValue::F32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as f32));
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
    /// 16-bit signed integers to an existing value.
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
    /// if the current value is not compatible with the insertion of integers,
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
    /// let mut value = dicom_value!(I16, [1, 2]);
    /// value.extend_i16([-5])?;
    /// assert_eq!(value.to_multi_int::<i16>()?, vec![1, 2, -5]);
    ///
    /// let mut value = dicom_value!(Strs, ["City"]);
    /// value.extend_i16([17])?;
    /// assert_eq!(value.to_string(), "City\\17");
    /// # Ok(())
    /// # }
    /// ```
    pub fn extend_i16(
        &mut self,
        numbers: impl IntoIterator<Item = i16>,
    ) -> Result<(), ModifyValueError> {
        match self {
            PrimitiveValue::Empty => {
                *self = PrimitiveValue::I16(numbers.into_iter().collect());
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
            PrimitiveValue::I16(elements) => {
                elements.extend(numbers);
                Ok(())
            }
            PrimitiveValue::U8(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u8));
                Ok(())
            }
            PrimitiveValue::U16(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u16));
                Ok(())
            }
            PrimitiveValue::U32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u32));
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
            PrimitiveValue::U64(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u64));
                Ok(())
            }
            PrimitiveValue::F32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as f32));
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
    /// 32-bit signed integers to an existing value.
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
    /// if the current value is not compatible with the insertion of integers,
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
    /// let mut value = dicom_value!(I32, [1, 2]);
    /// value.extend_i32([5])?;
    /// assert_eq!(value.to_multi_int::<i32>()?, vec![1, 2, 5]);
    ///
    /// let mut value = dicom_value!(Strs, ["City"]);
    /// value.extend_i32([17])?;
    /// assert_eq!(value.to_string(), "City\\17");
    /// # Ok(())
    /// # }
    /// ```
    pub fn extend_i32(
        &mut self,
        numbers: impl IntoIterator<Item = i32>,
    ) -> Result<(), ModifyValueError> {
        match self {
            PrimitiveValue::Empty => {
                *self = PrimitiveValue::I32(numbers.into_iter().collect());
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
            PrimitiveValue::I32(elements) => {
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
            PrimitiveValue::U32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u32));
                Ok(())
            }
            PrimitiveValue::I64(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as i64));
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
    /// 32-bit unsigned integers to an existing value.
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
    /// if the current value is not compatible with the insertion of integers,
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
    /// let mut value = dicom_value!(U32, [1, 2]);
    /// value.extend_u32([5])?;
    /// assert_eq!(value.to_multi_int::<u32>()?, vec![1, 2, 5]);
    ///
    /// let mut value = dicom_value!(Strs, ["City"]);
    /// value.extend_u32([17])?;
    /// assert_eq!(value.to_string(), "City\\17");
    /// # Ok(())
    /// # }
    /// ```
    pub fn extend_u32(
        &mut self,
        numbers: impl IntoIterator<Item = u32>,
    ) -> Result<(), ModifyValueError> {
        match self {
            PrimitiveValue::Empty => {
                *self = PrimitiveValue::U32(numbers.into_iter().collect());
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
            PrimitiveValue::U32(elements) => {
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
            PrimitiveValue::U64(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as u64));
                Ok(())
            }
            PrimitiveValue::F32(elements) => {
                elements.extend(numbers.into_iter().map(|n| n as f32));
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
}

#[cfg(test)]
mod tests {
    use crate::{
        dicom_value,
        value::{ConvertValueError, InvalidValueReadError, ValueType},
    };

    use super::super::PrimitiveValue;

    #[test]
    fn primitive_value_to_int() {
        assert!(PrimitiveValue::Empty.to_int::<i32>().is_err());

        // exact match
        assert_eq!(
            PrimitiveValue::from(0x0601_u16).to_int().ok(),
            Some(0x0601_u16),
        );
        // conversions are automatically applied
        assert_eq!(
            PrimitiveValue::from(0x0601_u16).to_int().ok(),
            Some(0x0601_u32),
        );
        assert_eq!(
            PrimitiveValue::from(0x0601_u16).to_int().ok(),
            Some(0x0601_i64),
        );
        assert_eq!(
            PrimitiveValue::from(0x0601_u16).to_int().ok(),
            Some(0x0601_u64),
        );

        // takes the first number
        assert_eq!(dicom_value!(I32, [1, 2, 5]).to_int().ok(), Some(1),);

        // admits an integer as text
        assert_eq!(dicom_value!(Strs, ["-73", "2"]).to_int().ok(), Some(-73),);

        // admits an integer as text with leading spaces
        assert_eq!(dicom_value!(Strs, [" -73", " 2"]).to_int().ok(), Some(-73),);

        // does not admit destructive conversions
        assert!(PrimitiveValue::from(-1).to_int::<u32>().is_err());

        // does not admit strings which are not numbers
        assert!(matches!(
            dicom_value!(Strs, ["Smith^John"]).to_int::<u8>(),
            Err(ConvertValueError {
                requested: _,
                original: ValueType::Strs,
                // would try to parse as an integer and fail
                cause: Some(cause),
            }) if matches!(&*cause, InvalidValueReadError::ParseInteger { .. })
        ));
    }

    #[test]
    fn primitive_value_to_multi_int() {
        assert_eq!(PrimitiveValue::Empty.to_multi_int::<i32>().unwrap(), vec![]);

        let test_value = dicom_value!(U16, [0x0601, 0x5353, 3, 4]);
        // exact match
        let numbers = test_value.to_multi_int::<u16>().unwrap();
        assert_eq!(numbers, vec![0x0601, 0x5353, 3, 4],);
        // type is inferred on context
        let numbers: Vec<u32> = test_value.to_multi_int().unwrap();
        assert_eq!(numbers, vec![0x0601_u32, 0x5353, 3, 4],);
        let numbers: Vec<i64> = test_value.to_multi_int().unwrap();
        assert_eq!(numbers, vec![0x0601_i64, 0x5353, 3, 4],);
        assert_eq!(
            test_value.to_multi_int::<u64>().unwrap(),
            vec![0x0601_u64, 0x5353, 3, 4],
        );

        // takes all numbers
        assert_eq!(
            dicom_value!(I32, [1, 2, 5]).to_multi_int().ok(),
            Some(vec![1, 2, 5]),
        );

        // admits a integer as text, trailing space too
        assert_eq!(
            dicom_value!(Strs, ["-73", "2 "]).to_multi_int().ok(),
            Some(vec![-73, 2]),
        );

        // does not admit destructive conversions
        assert!(matches!(
            dicom_value!(I32, [0, 1, -1]).to_multi_int::<u64>(),
            Err(ConvertValueError {
                original: ValueType::I32,
                // the cast from -1_i32 to u32 would fail
                cause: Some(cause),
                ..
            }) if matches!(&*cause,
                InvalidValueReadError::NarrowConvert {
                value: x,
               ..
            } if x == "-1")
        ));

        // not even from strings
        assert!(matches!(
            dicom_value!(Strs, ["0", "1", "-1"]).to_multi_int::<u16>(),
            Err(ConvertValueError {
                original: ValueType::Strs,
                // the conversion from "-1" to u32 would fail
                cause: Some(cause),
                ..
            }) if matches!(&*cause, InvalidValueReadError::ParseInteger { .. })
        ));

        // does not admit strings which are not numbers
        assert!(matches!(
            dicom_value!(Strs, ["Smith^John"]).to_int::<u8>(),
            Err(ConvertValueError {
                requested: _,
                original: ValueType::Strs,
                // would try to parse as an integer and fail
                cause: Some(cause),
            }) if matches!(&*cause, InvalidValueReadError::ParseInteger { .. })
        ));
    }
}
