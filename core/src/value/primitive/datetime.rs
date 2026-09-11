//! Date/time conversion implementation for `PrimitiveValue`

use super::*;
use crate::value::{ConvertValueError, deserialize, range};

impl PrimitiveValue {
    /// Retrieve a single [`chrono::NaiveDate`] from this value.
    ///
    /// Please note, that this is a shortcut to obtain a usable date from a primitive value.
    /// As per standard, the stored value might not be precise. It is highly recommended to
    /// use [`to_date`] as the only way to obtain dates.
    ///
    /// If the value is already represented as a precise [`DicomDate`], it is converted
    ///  to a [`NaiveDate`] value. It fails for imprecise values.
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a date, potentially failing if the
    /// string does not represent a valid date.
    /// If the value is a sequence of U8 bytes, the bytes are
    /// first interpreted as an ASCII character string.
    ///
    /// Users are advised that this method is DICOM compliant and a full
    /// date representation of YYYYMMDD is required. Otherwise, the operation fails.
    ///
    /// Partial precision dates are handled by [`DicomDate`], which can be retrieved
    /// by [`to_date`].
    ///
    /// [`to_date`]: Self::to_date
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue, DicomDate};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveDate;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Date(smallvec![
    ///         DicomDate::from_ymd(2014, 10, 12)?,
    ///     ])
    ///     .to_naive_date().ok(),
    ///     Some(NaiveDate::from_ymd(2014, 10, 12)),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Strs(smallvec![
    ///         "20141012".to_string(),
    ///     ])
    ///     .to_naive_date().ok(),
    ///     Some(NaiveDate::from_ymd(2014, 10, 12)),
    /// );
    ///
    /// assert!(
    ///     PrimitiveValue::Str("201410".to_string())
    ///     .to_naive_date().is_err()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_naive_date(&self) -> Result<NaiveDate, ConvertValueError> {
        match self {
            PrimitiveValue::Date(v) if !v.is_empty() => v[0]
                .to_naive_date()
                .context(ParseDateRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveDate",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Str(s) => deserialize::parse_date(s.as_bytes())
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveDate",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Strs(s) => {
                deserialize::parse_date(s.first().map(|s| s.as_bytes()).unwrap_or(&[]))
                    .context(ParseDateSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "NaiveDate",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::U8(bytes) => deserialize::parse_date(bytes)
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveDate",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "NaiveDate",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve the full sequence of [`chrono::NaiveDate`]s from this value.
    ///
    /// Please note, that this is a shortcut to obtain usable dates from a primitive value.
    /// As per standard, the stored values might not be precise. It is highly recommended to
    /// use [`to_multi_date`] as the only way to obtain dates.
    ///
    /// If the value is already represented as a sequence of precise [`DicomDate`] values,
    /// it is converted. It fails for imprecise values.
    /// If the value is a string or sequence of strings,
    /// the strings are decoded to obtain a date, potentially failing if
    /// any of the strings does not represent a valid date.
    /// If the value is a sequence of [`U8`] bytes, the bytes are
    /// first interpreted as an ASCII character string,
    /// then as a backslash-separated list of dates.
    ///
    /// Users are advised that this method is DICOM compliant and a full
    /// date representation of YYYYMMDD is required. Otherwise, the operation fails.
    ///
    /// Partial precision dates are handled by [`DicomDate`], which can be retrieved
    /// by [`to_multi_date`].
    ///
    /// [`U8`]: Self::U8
    /// [`to_multi_date`]: Self::to_multi_date
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue, DicomDate};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveDate;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Date(smallvec![
    ///         DicomDate::from_ymd(2014, 10, 12)?,
    ///     ]).to_multi_naive_date().ok(),
    ///     Some(vec![NaiveDate::from_ymd(2014, 10, 12)]),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Strs(smallvec![
    ///         "20141012".to_string(),
    ///         "20200828".to_string(),
    ///     ]).to_multi_naive_date().ok(),
    ///     Some(vec![
    ///         NaiveDate::from_ymd(2014, 10, 12),
    ///         NaiveDate::from_ymd(2020, 8, 28),
    ///     ]),
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_multi_naive_date(&self) -> Result<Vec<NaiveDate>, ConvertValueError> {
        match self {
            PrimitiveValue::Date(v) if !v.is_empty() => v
                .into_iter()
                .map(|d| d.to_naive_date())
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveDate",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Str(s) => {
                deserialize::parse_date(s.trim_end_matches(whitespace_or_null).as_bytes())
                    .map(|date| vec![date])
                    .context(ParseDateSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "NaiveDate",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::Strs(s) => s
                .into_iter()
                .map(|s| deserialize::parse_date(s.trim_end_matches(whitespace_or_null).as_bytes()))
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveDate",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::U8(bytes) => trim_last_whitespace(bytes)
                .split(|c| *c == b'\\')
                .map(deserialize::parse_date)
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveDate",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "NaiveDate",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a single [`DicomDate`] from this value.
    ///
    /// If the value is already represented as a [`DicomDate`], it is returned.
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a [`DicomDate`], potentially failing if the
    /// string does not represent a valid [`DicomDate`].
    /// If the value is a sequence of [`U8`] bytes, the bytes are
    /// first interpreted as an ASCII character string.
    ///
    /// Unlike Rust's [`chrono::NaiveDate`], [`DicomDate`] allows for missing date components.
    /// [`DicomDate`] implements [`AsRange`] trait, so specific
    /// [`chrono::NaiveDate`] values can be retrieved.
    ///
    /// - [`AsRange::exact`]
    /// - [`AsRange::earliest`]
    /// - [`AsRange::latest`]
    /// - [`AsRange::range`]
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveDate;
    /// # use std::error::Error;
    /// use dicom_core::value::{AsRange, DicomDate};
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    ///  let value = PrimitiveValue::Str("200002".into());
    ///  let date = value.to_date()?;
    ///
    ///  // it is not precise, day of month is unspecified
    ///  assert_eq!(
    ///     date.is_precise(),
    ///     false
    ///     );
    ///  assert_eq!(
    ///     date.earliest()?,
    ///     NaiveDate::from_ymd(2000,2,1)
    ///     );
    ///  assert_eq!(
    ///     date.latest()?,
    ///     NaiveDate::from_ymd(2000,2,29)
    ///     );
    ///  assert!(date.exact().is_err());
    ///
    ///  let date = PrimitiveValue::Str("20000201".into()).to_date()?;
    ///  assert_eq!(
    ///     date.is_precise(),
    ///     true
    ///     );
    ///  // .to_naive_date() works only for precise values
    ///  assert_eq!(
    ///     date.exact()?,
    ///     date.to_naive_date()?
    ///  );
    /// # Ok(())
    /// # }
    ///
    /// ```
    pub fn to_date(&self) -> Result<DicomDate, ConvertValueError> {
        match self {
            PrimitiveValue::Date(d) if !d.is_empty() => Ok(d[0]),
            PrimitiveValue::Str(s) => deserialize::parse_date_partial(s.as_bytes())
                .map(|(date, _)| date)
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DicomDate",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Strs(s) => {
                deserialize::parse_date_partial(s.first().map(|s| s.as_bytes()).unwrap_or(&[]))
                    .map(|(date, _)| date)
                    .context(ParseDateSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "DicomDate",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::U8(bytes) => deserialize::parse_date_partial(bytes)
                .map(|(date, _)| date)
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DicomDate",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "DicomDate",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve the full sequence of [`DicomDate`]s from this value.
    ///
    /// # Example
    /// ```
    /// # use dicom_core::value::{PrimitiveValue};
    /// # use dicom_core::dicom_value;
    /// use dicom_core::value::DicomDate;
    /// # use std::error::Error;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// assert_eq!(
    ///     dicom_value!(Strs, ["201410", "2020", "20200101"])
    ///         .to_multi_date()?,
    ///     vec![
    ///         DicomDate::from_ym(2014, 10)?,
    ///         DicomDate::from_y(2020)?,
    ///         DicomDate::from_ymd(2020, 1, 1)?
    ///     ]);
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    pub fn to_multi_date(&self) -> Result<Vec<DicomDate>, ConvertValueError> {
        match self {
            PrimitiveValue::Date(d) => Ok(d.to_vec()),
            PrimitiveValue::Str(s) => {
                deserialize::parse_date_partial(s.trim_end_matches(whitespace_or_null).as_bytes())
                    .map(|(date, _)| vec![date])
                    .context(ParseDateSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "DicomDate",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::Strs(s) => s
                .into_iter()
                .map(|s| {
                    deserialize::parse_date_partial(
                        s.trim_end_matches(whitespace_or_null).as_bytes(),
                    )
                    .map(|(date, _rest)| date)
                })
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DicomDate",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::U8(bytes) => trim_last_whitespace(bytes)
                .split(|c| *c == b'\\')
                .map(|s| deserialize::parse_date_partial(s).map(|(date, _rest)| date))
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DicomDate",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "DicomDate",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a single [`chrono::NaiveTime`] from this value.
    ///
    /// Please note, that this is a shortcut to obtain a usable time from a primitive value.
    /// As per standard, the stored value might not be precise. It is highly recommended to
    /// use [`Self::to_time`] as the only way to obtain times.
    ///
    /// If the value is represented as a precise [`DicomTime`],
    /// it is converted to a [`NaiveTime`].
    /// It fails for imprecise values,
    /// as in, those which do not specify up to at least the seconds.
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a time, potentially failing if the
    /// string does not represent a valid time.
    /// If the value is a sequence of [`U8`] bytes, the bytes are
    /// first interpreted as an ASCII character string.
    /// Otherwise, the operation fails.
    ///
    /// Partial precision times are handled by [`DicomTime`],
    /// which can be retrieved by [`to_time`].
    ///
    /// [`U8`]: Self::U8
    /// [`to_time`]: Self::to_time
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue, DicomTime};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveTime;
    /// # use std::error::Error;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from(DicomTime::from_hms(11, 2, 45)?).to_naive_time().ok(),
    ///     Some(NaiveTime::from_hms(11, 2, 45)),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("110245.78").to_naive_time().ok(),
    ///     Some(NaiveTime::from_hms_milli(11, 2, 45, 780)),
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_naive_time(&self) -> Result<NaiveTime, ConvertValueError> {
        match self {
            PrimitiveValue::Time(v) if !v.is_empty() => v[0]
                .to_naive_time()
                .context(ParseTimeRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveTime",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Str(s) => {
                deserialize::parse_time(s.trim_end_matches(whitespace_or_null).as_bytes())
                    .map(|(date, _rest)| date)
                    .context(ParseTimeSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "NaiveTime",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::Strs(s) => deserialize::parse_time(
                s.first()
                    .map(|s| s.trim_end_matches(whitespace_or_null).as_bytes())
                    .unwrap_or(&[]),
            )
            .map(|(date, _rest)| date)
            .context(ParseTimeSnafu)
            .map_err(|err| ConvertValueError {
                requested: "NaiveTime",
                original: self.value_type(),
                cause: Some(Box::from(err)),
            }),
            PrimitiveValue::U8(bytes) => deserialize::parse_time(trim_last_whitespace(bytes))
                .map(|(date, _rest)| date)
                .context(ParseTimeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveTime",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "NaiveTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve the full sequence of [`chrono::NaiveTime`]s from this value.
    ///
    /// Please note, that this is a shortcut to obtain a usable time from a primitive value.
    /// As per standard, the stored values might not be precise. It is highly recommended to
    /// use [`to_multi_time`] as the only way to obtain times.
    ///
    /// If the value is already represented as a sequence of precise [`DicomTime`] values,
    /// it is converted to a sequence of [`NaiveTime`] values. It fails for imprecise values.
    /// If the value is a string or sequence of strings,
    /// the strings are decoded to obtain a date, potentially failing if
    /// any of the strings does not represent a valid date.
    /// If the value is a sequence of [`U8`] bytes, the bytes are
    /// first interpreted as an ASCII character string,
    /// then as a backslash-separated list of times.
    /// Otherwise, the operation fails.
    ///
    /// Users are advised that this method requires at least 1 out of 6 digits of the second
    /// fraction `.F` to be present. Otherwise, the operation fails.
    ///
    /// Partial precision times are handled by [`DicomTime`],
    /// which can be retrieved by [`to_multi_time`].
    ///
    /// [`to_multi_time`]: Self::to_multi_time
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue, DicomTime};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveTime;
    /// # use std::error::Error;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from(DicomTime::from_hms(22, 58, 2)?).to_multi_naive_time().ok(),
    ///     Some(vec![NaiveTime::from_hms(22, 58, 2)]),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Strs(smallvec![
    ///         "225802.1".to_string(),
    ///         "225916.742388".to_string(),
    ///     ]).to_multi_naive_time().ok(),
    ///     Some(vec![
    ///         NaiveTime::from_hms_micro(22, 58, 2, 100_000),
    ///         NaiveTime::from_hms_micro(22, 59, 16, 742_388),
    ///     ]),
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_multi_naive_time(&self) -> Result<Vec<NaiveTime>, ConvertValueError> {
        match self {
            PrimitiveValue::Time(v) if !v.is_empty() => v
                .into_iter()
                .map(|t| t.to_naive_time())
                .collect::<Result<Vec<_>, _>>()
                .context(ParseTimeRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveTime",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Str(s) => {
                deserialize::parse_time(s.trim_end_matches(whitespace_or_null).as_bytes())
                    .map(|(date, _rest)| vec![date])
                    .context(ParseDateSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "NaiveTime",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::Strs(s) => s
                .into_iter()
                .map(|s| {
                    deserialize::parse_time(s.trim_end_matches(whitespace_or_null).as_bytes())
                        .map(|(date, _rest)| date)
                })
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveTime",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::U8(bytes) => trim_last_whitespace(bytes)
                .split(|c| *c == b'\\')
                .map(|s| deserialize::parse_time(s).map(|(date, _rest)| date))
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "NaiveTime",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "NaiveTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a single [`DicomTime`] from this value.
    ///
    /// If the value is already represented as a time, it is converted into [`DicomTime`].
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a [`DicomTime`], potentially failing if the
    /// string does not represent a valid [`DicomTime`].
    /// If the value is a sequence of [`U8`] bytes, the bytes are
    /// first interpreted as an ASCII character string.
    ///
    /// Unlike Rust's [`chrono::NaiveTime`], [`DicomTime`] allows for missing time components.
    /// [`DicomTime`] implements [`AsRange`] trait, so specific [`chrono::NaiveTime`] values can be retrieved.
    ///
    /// - [`AsRange::exact`]
    /// - [`AsRange::earliest`]
    /// - [`AsRange::latest`]
    /// - [`AsRange::range`]
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use chrono::NaiveTime;
    /// use dicom_core::value::{AsRange, DicomTime};
    /// # use std::error::Error;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    ///  let value = PrimitiveValue::Str("10".into());
    ///  let time = value.to_time()?;
    ///
    ///  // is not precise, minute, second and second fraction are unspecified
    ///  assert_eq!(
    ///     time.is_precise(),
    ///     false
    ///     );
    ///  assert_eq!(
    ///     time.earliest()?,
    ///     NaiveTime::from_hms(10,0,0)
    ///     );
    ///  assert_eq!(
    ///     time.latest()?,
    ///     NaiveTime::from_hms_micro(10,59,59,999_999)
    ///     );
    ///  assert!(time.exact().is_err());
    ///
    ///  let second = PrimitiveValue::Str("101259".into());
    ///  // not a precise value, fraction of second is unspecified
    ///  assert!(second.to_time()?.exact().is_err());
    ///
    ///  // .to_naive_time() yields a result, for at least second precision values
    ///  // second fraction defaults to zeros
    ///  assert_eq!(
    ///     second.to_time()?.to_naive_time()?,
    ///     NaiveTime::from_hms(10,12,59)
    ///  );
    ///
    ///  let fraction6 = PrimitiveValue::Str("101259.123456".into());
    ///  let fraction5 = PrimitiveValue::Str("101259.12345".into());
    ///
    ///  // is not precise, last digit of second fraction is unspecified
    ///  assert!(
    ///     fraction5.to_time()?.exact().is_err()
    ///  );
    ///  assert!(
    ///     fraction6.to_time()?.exact().is_ok()
    ///  );
    ///
    ///  assert_eq!(
    ///     fraction6.to_time()?.exact()?,
    ///     fraction6.to_time()?.to_naive_time()?
    ///  );
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_time(&self) -> Result<DicomTime, ConvertValueError> {
        match self {
            PrimitiveValue::Time(t) if !t.is_empty() => Ok(t[0]),
            PrimitiveValue::Str(s) => {
                deserialize::parse_time_partial(s.trim_end_matches(whitespace_or_null).as_bytes())
                    .map(|(date, _rest)| date)
                    .context(ParseTimeSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "DicomTime",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::Strs(s) => deserialize::parse_time_partial(
                s.first()
                    .map(|s| s.trim_end_matches(whitespace_or_null).as_bytes())
                    .unwrap_or(&[]),
            )
            .map(|(date, _rest)| date)
            .context(ParseTimeSnafu)
            .map_err(|err| ConvertValueError {
                requested: "DicomTime",
                original: self.value_type(),
                cause: Some(Box::from(err)),
            }),
            PrimitiveValue::U8(bytes) => {
                deserialize::parse_time_partial(trim_last_whitespace(bytes))
                    .map(|(date, _rest)| date)
                    .context(ParseTimeSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "DicomTime",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            _ => Err(ConvertValueError {
                requested: "DicomTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve the full sequence of [`DicomTime`]s from this value.
    ///
    /// If the value is already represented as a time, it is converted into [`DicomTime`].
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a [`DicomTime`], potentially failing if the
    /// string does not represent a valid [`DicomTime`].
    /// If the value is a sequence of [`U8`] bytes, the bytes are
    /// first interpreted as an ASCII character string.
    ///
    /// Unlike Rust's [`chrono::NaiveTime`], [`DicomTime`] allows for missing time components.
    /// [`DicomTime`] implements [`AsRange`] trait, so specific [`chrono::NaiveTime`] values can be retrieved.
    ///
    /// - [`AsRange::exact`]
    /// - [`AsRange::earliest`]
    /// - [`AsRange::latest`]
    /// - [`AsRange::range`]
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use std::error::Error;
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// use dicom_core::value::DicomTime;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Strs(smallvec![
    ///         "2258".to_string(),
    ///         "225916.000742".to_string(),
    ///     ]).to_multi_time()?,
    ///     vec![
    ///         DicomTime::from_hm(22, 58)?,
    ///         DicomTime::from_hms_micro(22, 59, 16, 742)?,
    ///     ],
    /// );
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_multi_time(&self) -> Result<Vec<DicomTime>, ConvertValueError> {
        match self {
            PrimitiveValue::Time(t) => Ok(t.to_vec()),
            PrimitiveValue::Str(s) => {
                deserialize::parse_time_partial(s.trim_end_matches(whitespace_or_null).as_bytes())
                    .map(|(date, _rest)| vec![date])
                    .context(ParseDateSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "DicomTime",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::Strs(s) => s
                .into_iter()
                .map(|s| {
                    deserialize::parse_time_partial(
                        s.trim_end_matches(whitespace_or_null).as_bytes(),
                    )
                    .map(|(date, _rest)| date)
                })
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DicomTime",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::U8(bytes) => trim_last_whitespace(bytes)
                .split(|c| *c == b'\\')
                .map(|s| deserialize::parse_time_partial(s).map(|(date, _rest)| date))
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DicomTime",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "DicomTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a single [`DicomDateTime`] from this value.
    ///
    /// If the value is already represented as a date-time, it is converted into [`DicomDateTime`].
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a [`DicomDateTime`], potentially failing if the
    /// string does not represent a valid [`DicomDateTime`].
    /// If the value is a sequence of [`U8`] bytes, the bytes are
    /// first interpreted as an ASCII character string.
    ///
    /// Unlike Rust's [`chrono::DateTime`], [`DicomDateTime`] allows for missing date or time components.
    /// [`DicomDateTime`] implements [`AsRange`] trait, so specific [`chrono::DateTime`] values can be retrieved.
    ///
    /// - [`AsRange::exact`]
    /// - [`AsRange::earliest`]
    /// - [`AsRange::latest`]
    /// - [`AsRange::range`]
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::{DateTime, FixedOffset, TimeZone, NaiveDateTime, NaiveDate, NaiveTime};
    /// # use std::error::Error;
    /// use dicom_core::value::{DicomDateTime, AsRange, DateTimeRange, PreciseDateTime};
    ///
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// // let's parse a date-time text value with 0.1 second precision without a time-zone.
    /// let dt_value = PrimitiveValue::from("20121221093001.1").to_datetime()?;
    ///
    /// assert_eq!(
    ///     dt_value.earliest()?,
    ///     PreciseDateTime::Naive(NaiveDateTime::new(
    ///      NaiveDate::from_ymd_opt(2012, 12, 21).unwrap(),
    ///      NaiveTime::from_hms_micro_opt(9, 30, 1, 100_000).unwrap()
    ///         ))
    /// );
    /// assert_eq!(
    ///     dt_value.latest()?,
    ///     PreciseDateTime::Naive(NaiveDateTime::new(
    ///      NaiveDate::from_ymd_opt(2012, 12, 21).unwrap(),
    ///      NaiveTime::from_hms_micro_opt(9, 30, 1, 199_999).unwrap()
    ///         ))
    /// );
    ///
    /// let default_offset = FixedOffset::east_opt(3600).unwrap();
    /// // let's parse a date-time text value with full precision with a time-zone east +01:00.
    /// let dt_value = PrimitiveValue::from("20121221093001.123456+0100").to_datetime()?;
    ///
    /// // date-time has all components
    /// assert_eq!(dt_value.is_precise(), true);
    ///
    /// assert_eq!(
    ///     dt_value.exact()?,
    ///     PreciseDateTime::TimeZone(
    ///     default_offset
    ///     .ymd_opt(2012, 12, 21).unwrap()
    ///     .and_hms_micro_opt(9, 30, 1, 123_456).unwrap()
    ///     )
    ///
    /// );
    ///
    /// // ranges are inclusive, for a precise value, two identical values are returned
    /// assert_eq!(
    ///     dt_value.range()?,
    ///     DateTimeRange::from_start_to_end_with_time_zone(
    ///         FixedOffset::east_opt(3600).unwrap()
    ///             .ymd_opt(2012, 12, 21).unwrap()
    ///             .and_hms_micro_opt(9, 30, 1, 123_456).unwrap(),
    ///         FixedOffset::east_opt(3600).unwrap()
    ///             .ymd_opt(2012, 12, 21).unwrap()
    ///             .and_hms_micro_opt(9, 30, 1, 123_456).unwrap()
    ///     )?
    ///
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_datetime(&self) -> Result<DicomDateTime, ConvertValueError> {
        match self {
            PrimitiveValue::DateTime(v) if !v.is_empty() => Ok(v[0]),
            PrimitiveValue::Str(s) => deserialize::parse_datetime_partial(
                s.trim_end_matches(whitespace_or_null).as_bytes(),
            )
            .context(ParseDateTimeSnafu)
            .map_err(|err| ConvertValueError {
                requested: "DicomDateTime",
                original: self.value_type(),
                cause: Some(Box::from(err)),
            }),
            PrimitiveValue::Strs(s) => deserialize::parse_datetime_partial(
                s.first()
                    .map(|s| s.trim_end_matches(whitespace_or_null).as_bytes())
                    .unwrap_or(&[]),
            )
            .context(ParseDateTimeSnafu)
            .map_err(|err| ConvertValueError {
                requested: "DicomDateTime",
                original: self.value_type(),
                cause: Some(Box::from(err)),
            }),
            PrimitiveValue::U8(bytes) => {
                deserialize::parse_datetime_partial(trim_last_whitespace(bytes))
                    .context(ParseDateTimeSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "DicomDateTime",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            _ => Err(ConvertValueError {
                requested: "DicomDateTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve the full sequence of [`DicomDateTime`]s from this value.
    ///
    pub fn to_multi_datetime(&self) -> Result<Vec<DicomDateTime>, ConvertValueError> {
        match self {
            PrimitiveValue::DateTime(v) => Ok(v.to_vec()),
            PrimitiveValue::Str(s) => deserialize::parse_datetime_partial(
                s.trim_end_matches(whitespace_or_null).as_bytes(),
            )
            .map(|date| vec![date])
            .context(ParseDateSnafu)
            .map_err(|err| ConvertValueError {
                requested: "DicomDateTime",
                original: self.value_type(),
                cause: Some(Box::from(err)),
            }),
            PrimitiveValue::Strs(s) => s
                .into_iter()
                .map(|s| {
                    deserialize::parse_datetime_partial(
                        s.trim_end_matches(whitespace_or_null).as_bytes(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DicomDateTime",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::U8(bytes) => trim_last_whitespace(bytes)
                .split(|c| *c == b'\\')
                .map(deserialize::parse_datetime_partial)
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDateSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DicomDateTime",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "DicomDateTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }
    /// Retrieve a single [`DateRange`] from this value.
    ///
    /// If the value is already represented as a [`DicomDate`], it is converted into [`DateRange`].
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a [`DateRange`], potentially failing if the
    /// string does not represent a valid [`DateRange`].
    /// If the value is a sequence of [`U8`] bytes, the bytes are
    /// first interpreted as an ASCII character string.
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// use chrono::{NaiveDate};
    /// # use std::error::Error;
    /// use dicom_core::value::{DateRange};
    ///
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// let da_range = PrimitiveValue::from("2012-201305").to_date_range()?;
    ///
    /// assert_eq!(
    ///     da_range.start(),
    ///     Some(&NaiveDate::from_ymd(2012, 1, 1))
    /// );
    /// assert_eq!(
    ///     da_range.end(),
    ///     Some(&NaiveDate::from_ymd(2013, 05, 31))
    /// );
    ///
    /// let range_from = PrimitiveValue::from("2012-").to_date_range()?;
    ///
    /// assert!(range_from.end().is_none());
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_date_range(&self) -> Result<DateRange, ConvertValueError> {
        match self {
            PrimitiveValue::Date(da) if !da.is_empty() => da[0]
                .range()
                .context(ParseDateRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DateRange",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Str(s) => {
                range::parse_date_range(s.trim_end_matches(whitespace_or_null).as_bytes())
                    .context(ParseDateRangeSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "DateRange",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::Strs(s) => range::parse_date_range(
                s.first()
                    .map(|s| s.trim_end_matches(whitespace_or_null).as_bytes())
                    .unwrap_or(&[]),
            )
            .context(ParseDateRangeSnafu)
            .map_err(|err| ConvertValueError {
                requested: "DateRange",
                original: self.value_type(),
                cause: Some(Box::from(err)),
            }),
            PrimitiveValue::U8(bytes) => range::parse_date_range(trim_last_whitespace(bytes))
                .context(ParseDateRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DateRange",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "DateRange",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a single [`TimeRange`] from this value.
    ///
    /// If the value is already represented as a [`DicomTime`], it is converted into a [`TimeRange`].
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a [`TimeRange`], potentially failing if the
    /// string does not represent a valid [`DateRange`].
    /// If the value is a sequence of [`U8`] bytes, the bytes are
    /// first interpreted as an ASCII character string.
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// use chrono::{NaiveTime};
    /// # use std::error::Error;
    /// use dicom_core::value::{TimeRange};
    ///
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// let tm_range = PrimitiveValue::from("02-153000.123").to_time_range()?;
    ///
    /// // null components default to zeros
    /// assert_eq!(
    ///     tm_range.start(),
    ///     Some(&NaiveTime::from_hms(2, 0, 0))
    /// );
    ///
    /// // unspecified part of second fraction defaults to latest possible
    /// assert_eq!(
    ///     tm_range.end(),
    ///     Some(&NaiveTime::from_hms_micro(15, 30, 0, 123_999))
    /// );
    ///
    /// let range_from = PrimitiveValue::from("01-").to_time_range()?;
    ///
    /// assert!(range_from.end().is_none());
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_time_range(&self) -> Result<TimeRange, ConvertValueError> {
        match self {
            PrimitiveValue::Time(t) if !t.is_empty() => t[0]
                .range()
                .context(ParseTimeRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "TimeRange",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Str(s) => {
                range::parse_time_range(s.trim_end_matches(whitespace_or_null).as_bytes())
                    .context(ParseTimeRangeSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "TimeRange",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::Strs(s) => range::parse_time_range(
                s.first()
                    .map(|s| s.trim_end_matches(whitespace_or_null).as_bytes())
                    .unwrap_or(&[]),
            )
            .context(ParseTimeRangeSnafu)
            .map_err(|err| ConvertValueError {
                requested: "TimeRange",
                original: self.value_type(),
                cause: Some(Box::from(err)),
            }),
            PrimitiveValue::U8(bytes) => range::parse_time_range(trim_last_whitespace(bytes))
                .context(ParseTimeRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "TimeRange",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "TimeRange",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a single [`DateTimeRange`] from this value.
    ///
    /// If the value is already represented as a [`DicomDateTime`], it is
    /// converted into [`DateTimeRange`].
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a [`DateTimeRange`], potentially
    /// failing if the  string does not represent a valid [`DateTimeRange`].
    /// If the value is a sequence of [`U8`] bytes, the bytes are
    /// first interpreted as an ASCII character string.
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// use chrono::{DateTime, NaiveDate, NaiveTime, NaiveDateTime, FixedOffset, TimeZone, Local};
    /// # use std::error::Error;
    /// use dicom_core::value::{DateTimeRange, PreciseDateTime};
    ///
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// // let's parse a text representation of a date-time range, where the lower bound is a microsecond
    /// // precision value with a time-zone (east +05:00) and the upper bound is a minimum precision value
    /// // with a time-zone
    /// let dt_range = PrimitiveValue::from("19920101153020.123+0500-1993+0300").to_datetime_range()?;
    ///
    /// // lower bound of range is parsed into a PreciseDateTimeResult::TimeZone variant
    /// assert_eq!(
    ///     dt_range.start(),
    ///     Some(PreciseDateTime::TimeZone(
    ///         FixedOffset::east_opt(5*3600).unwrap().ymd_opt(1992, 1, 1).unwrap()
    ///         .and_hms_micro_opt(15, 30, 20, 123_000).unwrap()
    ///         )
    ///     )
    /// );
    ///
    /// // upper bound of range is parsed into a PreciseDateTimeResult::TimeZone variant
    /// assert_eq!(
    ///     dt_range.end(),
    ///     Some(PreciseDateTime::TimeZone(
    ///         FixedOffset::east_opt(3*3600).unwrap().ymd_opt(1993, 12, 31).unwrap()
    ///         .and_hms_micro_opt(23, 59, 59, 999_999).unwrap()
    ///         )
    ///     )
    /// );
    ///
    /// let lower = PrimitiveValue::from("2012-").to_datetime_range()?;
    ///
    /// // range has no upper bound
    /// assert!(lower.end().is_none());
    ///
    /// // One time-zone in a range is missing
    /// let dt_range = PrimitiveValue::from("1992+0500-1993").to_datetime_range()?;
    ///
    /// // It will be replaced with the local clock time-zone offset
    /// // This can be customized with [to_datetime_range_custom()]
    /// assert_eq!(
    ///   dt_range,
    ///   DateTimeRange::TimeZone{
    ///         start: Some(FixedOffset::east_opt(5*3600).unwrap()
    ///             .ymd_opt(1992, 1, 1).unwrap()
    ///             .and_hms_micro_opt(0, 0, 0, 0).unwrap()
    ///         ),
    ///         end: Some(Local::now().offset()
    ///             .ymd_opt(1993, 12, 31).unwrap()
    ///             .and_hms_micro_opt(23, 59, 59, 999_999).unwrap()
    ///         )
    ///     }
    /// );
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_datetime_range(&self) -> Result<DateTimeRange, ConvertValueError> {
        match self {
            PrimitiveValue::DateTime(dt) if !dt.is_empty() => dt[0]
                .range()
                .context(ParseDateTimeRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DateTimeRange",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Str(s) => {
                range::parse_datetime_range(s.trim_end_matches(whitespace_or_null).as_bytes())
                    .context(ParseDateTimeRangeSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "DateTimeRange",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            PrimitiveValue::Strs(s) => range::parse_datetime_range(
                s.first()
                    .map(|s| s.trim_end_matches(whitespace_or_null).as_bytes())
                    .unwrap_or(&[]),
            )
            .context(ParseDateTimeRangeSnafu)
            .map_err(|err| ConvertValueError {
                requested: "DateTimeRange",
                original: self.value_type(),
                cause: Some(Box::from(err)),
            }),
            PrimitiveValue::U8(bytes) => range::parse_datetime_range(trim_last_whitespace(bytes))
                .context(ParseDateTimeRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DateTimeRange",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            _ => Err(ConvertValueError {
                requested: "DateTimeRange",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a single [`DateTimeRange`] from this value.
    ///
    /// Use a custom ambiguous date-time range parser.
    ///
    /// For full description see [`to_datetime_range`] and
    /// [`AmbiguousDtRangeParser`].
    ///
    /// [`to_datetime_range`]: Self::to_datetime_range
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use std::error::Error;
    /// use dicom_core::value::range::{AmbiguousDtRangeParser, ToKnownTimeZone, IgnoreTimeZone, FailOnAmbiguousRange, DateTimeRange};
    /// use chrono::{NaiveDate, NaiveTime, NaiveDateTime};
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// // The upper bound time-zone is missing
    /// // the default behavior in this case is to use the local clock time-zone.
    /// // But we want to use the known (parsed) time-zone from the lower bound instead.
    /// let dt_range = PrimitiveValue::from("1992+0500-1993")
    ///     .to_datetime_range_custom::<ToKnownTimeZone>()?;
    ///
    /// // values are in the same time-zone
    /// assert_eq!(
    ///     dt_range.start().unwrap()
    ///         .as_datetime().unwrap()
    ///         .offset(),
    ///     dt_range.end().unwrap()
    ///         .as_datetime().unwrap()
    ///         .offset()
    /// );
    ///
    /// // ignore parsed time-zone, retrieve a time-zone naive range
    /// let naive_range = PrimitiveValue::from("1992+0599-1993")
    ///     .to_datetime_range_custom::<IgnoreTimeZone>()?;
    ///
    /// assert_eq!(
    ///     naive_range,
    ///     DateTimeRange::from_start_to_end(
    ///         NaiveDateTime::new(
    ///             NaiveDate::from_ymd_opt(1992, 1, 1).unwrap(),
    ///             NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
    ///         ),
    ///         NaiveDateTime::new(
    ///             NaiveDate::from_ymd_opt(1993, 12, 31).unwrap(),
    ///             NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
    ///         )
    ///     ).unwrap()
    /// );
    ///
    /// // always fail upon parsing an ambiguous DT range
    /// assert!(
    /// PrimitiveValue::from("1992+0599-1993")
    ///     .to_datetime_range_custom::<FailOnAmbiguousRange>().is_err()
    /// );
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_datetime_range_custom<T: AmbiguousDtRangeParser>(
        &self,
    ) -> Result<DateTimeRange, ConvertValueError> {
        match self {
            PrimitiveValue::DateTime(dt) if !dt.is_empty() => dt[0]
                .range()
                .context(ParseDateTimeRangeSnafu)
                .map_err(|err| ConvertValueError {
                    requested: "DateTimeRange",
                    original: self.value_type(),
                    cause: Some(Box::from(err)),
                }),
            PrimitiveValue::Str(s) => range::parse_datetime_range_custom::<T>(
                s.trim_end_matches(whitespace_or_null).as_bytes(),
            )
            .context(ParseDateTimeRangeSnafu)
            .map_err(|err| ConvertValueError {
                requested: "DateTimeRange",
                original: self.value_type(),
                cause: Some(Box::from(err)),
            }),
            PrimitiveValue::Strs(s) => range::parse_datetime_range_custom::<T>(
                s.first()
                    .map(|s| s.trim_end_matches(whitespace_or_null).as_bytes())
                    .unwrap_or(&[]),
            )
            .context(ParseDateTimeRangeSnafu)
            .map_err(|err| ConvertValueError {
                requested: "DateTimeRange",
                original: self.value_type(),
                cause: Some(Box::from(err)),
            }),
            PrimitiveValue::U8(bytes) => {
                range::parse_datetime_range_custom::<T>(trim_last_whitespace(bytes))
                    .context(ParseDateTimeRangeSnafu)
                    .map_err(|err| ConvertValueError {
                        requested: "DateTimeRange",
                        original: self.value_type(),
                        cause: Some(Box::from(err)),
                    })
            }
            _ => Err(ConvertValueError {
                requested: "DateTimeRange",
                original: self.value_type(),
                cause: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone as _};
    use smallvec::smallvec;
    use crate::{dicom_value, value::{ConvertValueError, DateRange, DateTimeRange, DicomDate, DicomDateTime, DicomTime, TimeRange, ValueType, primitive::PrimitiveValue}};

    #[test]
    fn primitive_value_to_naive_date() {
        // to NaiveDate
        assert_eq!(
            PrimitiveValue::Date(smallvec![DicomDate::from_ymd(2014, 10, 12).unwrap()])
                .to_naive_date()
                .unwrap(),
            NaiveDate::from_ymd_opt(2014, 10, 12).unwrap(),
        );
        // from text (Str)
        assert_eq!(
            dicom_value!(Str, "20141012").to_naive_date().unwrap(),
            NaiveDate::from_ymd_opt(2014, 10, 12).unwrap(),
        );
        // from text (Strs)
        assert_eq!(
            dicom_value!(Strs, ["20141012"]).to_naive_date().unwrap(),
            NaiveDate::from_ymd_opt(2014, 10, 12).unwrap(),
        );
        // from bytes
        assert_eq!(
            PrimitiveValue::from(b"20200229").to_naive_date().unwrap(),
            NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
        );
        // not a date
        assert!(matches!(
            PrimitiveValue::Str("Smith^John".to_string()).to_naive_date(),
            Err(ConvertValueError {
                requested: "NaiveDate",
                original: ValueType::Str,
                // would try to parse as a date and fail
                cause: Some(_),
            })
        ));
    }

    #[test]
    fn primitive_value_to_dicom_date() {
        // primitive conversion
        assert_eq!(
            PrimitiveValue::Date(smallvec![DicomDate::from_ymd(2014, 10, 12).unwrap()])
                .to_date()
                .ok(),
            Some(DicomDate::from_ymd(2014, 10, 12).unwrap()),
        );

        // from Strs
        assert_eq!(
            dicom_value!(Strs, ["201410", "2020", "20200101"])
                .to_date()
                .unwrap(),
            DicomDate::from_ym(2014, 10).unwrap()
        );

        // from bytes
        assert_eq!(
            PrimitiveValue::from(b"202002").to_date().ok(),
            Some(DicomDate::from_ym(2020, 2).unwrap())
        );
    }

    #[test]
    fn primitive_value_to_multi_dicom_date() {
        assert_eq!(
            dicom_value!(Strs, ["201410", "2020", "20200101"])
                .to_multi_date()
                .unwrap(),
            vec![
                DicomDate::from_ym(2014, 10).unwrap(),
                DicomDate::from_y(2020).unwrap(),
                DicomDate::from_ymd(2020, 1, 1).unwrap()
            ]
        );

        assert!(dicom_value!(Strs, ["-44"]).to_multi_date().is_err());
    }

    #[test]
    fn primitive_value_to_naive_time() {
        // trivial conversion
        assert_eq!(
            PrimitiveValue::from(DicomTime::from_hms(11, 9, 26).unwrap())
                .to_naive_time()
                .unwrap(),
            NaiveTime::from_hms_opt(11, 9, 26).unwrap(),
        );
        // from text (Str)
        assert_eq!(
            dicom_value!(Str, "110926.3").to_naive_time().unwrap(),
            NaiveTime::from_hms_milli_opt(11, 9, 26, 300).unwrap(),
        );
        // from text with fraction of a second + padding
        assert_eq!(
            PrimitiveValue::from("110926.38 ").to_naive_time().unwrap(),
            NaiveTime::from_hms_milli_opt(11, 9, 26, 380).unwrap(),
        );
        // from text (Strs)
        assert_eq!(
            dicom_value!(Strs, ["110926.38"]).to_naive_time().unwrap(),
            NaiveTime::from_hms_milli_opt(11, 9, 26, 380).unwrap(),
        );

        // from text without fraction of a second (assumes 0 ms in fraction)
        assert_eq!(
            dicom_value!(Str, "110926").to_naive_time().unwrap(),
            NaiveTime::from_hms_opt(11, 9, 26).unwrap(),
        );

        // absence of seconds is considered to be an incomplete value
        assert!(PrimitiveValue::from("1109").to_naive_time().is_err(),);
        assert!(dicom_value!(Strs, ["1109"]).to_naive_time().is_err());
        assert!(dicom_value!(Strs, ["11"]).to_naive_time().is_err());

        // not a time
        assert!(matches!(
            PrimitiveValue::Str("Smith^John".to_string()).to_naive_time(),
            Err(ConvertValueError {
                requested: "NaiveTime",
                original: ValueType::Str,
                ..
            })
        ));
    }

    #[test]
    fn primitive_value_to_dicom_time() {
        // from NaiveTime - results in exact DicomTime with default fraction
        assert_eq!(
            PrimitiveValue::from(DicomTime::from_hms_micro(11, 9, 26, 0).unwrap())
                .to_time()
                .unwrap(),
            DicomTime::from_hms_micro(11, 9, 26, 0).unwrap(),
        );
        // from NaiveTime with milli precision
        assert_eq!(
            PrimitiveValue::from(DicomTime::from_hms_milli(11, 9, 26, 123).unwrap())
                .to_time()
                .unwrap(),
            DicomTime::from_hms_milli(11, 9, 26, 123).unwrap(),
        );
        // from NaiveTime with micro precision
        assert_eq!(
            PrimitiveValue::from(DicomTime::from_hms_micro(11, 9, 26, 123).unwrap())
                .to_time()
                .unwrap(),
            DicomTime::from_hms_micro(11, 9, 26, 123).unwrap(),
        );
        // from text (Str)
        assert_eq!(
            dicom_value!(Str, "110926").to_time().unwrap(),
            DicomTime::from_hms(11, 9, 26).unwrap(),
        );
        // from text with fraction of a second + padding
        assert_eq!(
            PrimitiveValue::from("110926.38 ").to_time().unwrap(),
            DicomTime::from_hmsf(11, 9, 26, 38, 2).unwrap(),
        );
        // from text (Strs)
        assert_eq!(
            dicom_value!(Strs, ["110926"]).to_time().unwrap(),
            DicomTime::from_hms(11, 9, 26).unwrap(),
        );
        // from text (Strs) with fraction of a second
        assert_eq!(
            dicom_value!(Strs, ["110926.123456"]).to_time().unwrap(),
            DicomTime::from_hms_micro(11, 9, 26, 123_456).unwrap(),
        );
        // from bytes with fraction of a second
        assert_eq!(
            PrimitiveValue::from(&b"110926.987"[..]).to_time().unwrap(),
            DicomTime::from_hms_milli(11, 9, 26, 987).unwrap(),
        );
        // from bytes with fraction of a second + padding
        assert_eq!(
            PrimitiveValue::from(&b"110926.38 "[..]).to_time().unwrap(),
            DicomTime::from_hmsf(11, 9, 26, 38, 2).unwrap(),
        );
        // not a time
        assert!(matches!(
            PrimitiveValue::Str("Smith^John".to_string()).to_time(),
            Err(ConvertValueError {
                requested: "DicomTime",
                original: ValueType::Str,
                ..
            })
        ));
    }

    #[test]
    fn primitive_value_to_dicom_datetime() {
        let offset = FixedOffset::east_opt(1).unwrap();

        // try from chrono::DateTime<FixedOffset>
        assert_eq!(
            PrimitiveValue::from(
                DicomDateTime::from_date_and_time_with_time_zone(
                    DicomDate::from_ymd(2012, 12, 21).unwrap(),
                    DicomTime::from_hms_micro(11, 9, 26, /* 000 */ 123).unwrap(),
                    offset
                )
                .unwrap()
            )
            .to_datetime()
            .unwrap(),
            DicomDateTime::from_date_and_time_with_time_zone(
                DicomDate::from_ymd(2012, 12, 21).unwrap(),
                DicomTime::from_hms_micro(11, 9, 26, /* 000 */ 123).unwrap(),
                offset
            )
            .unwrap()
        );
        // try from chrono::NaiveDateTime
        assert_eq!(
            PrimitiveValue::from(
                DicomDateTime::from_date_and_time(
                    DicomDate::from_ymd(2012, 12, 21).unwrap(),
                    DicomTime::from_hms_micro(11, 9, 26, /* 000 */ 123).unwrap()
                )
                .unwrap()
            )
            .to_datetime()
            .unwrap(),
            DicomDateTime::from_date_and_time(
                DicomDate::from_ymd(2012, 12, 21).unwrap(),
                DicomTime::from_hms_micro(11, 9, 26, /* 000 */ 123).unwrap()
            )
            .unwrap()
        );
        // from text (Str) - minimum allowed is a YYYY
        assert_eq!(
            dicom_value!(Str, "2012").to_datetime().unwrap(),
            DicomDateTime::from_date(DicomDate::from_y(2012).unwrap())
        );
        // from text with fraction of a second + padding
        assert_eq!(
            PrimitiveValue::from("20121221110926.38 ")
                .to_datetime()
                .unwrap(),
            DicomDateTime::from_date_and_time(
                DicomDate::from_ymd(2012, 12, 21).unwrap(),
                DicomTime::from_hmsf(11, 9, 26, 38, 2).unwrap()
            )
            .unwrap()
        );
        // from text (Strs) with fraction of a second + padding
        assert_eq!(
            dicom_value!(Strs, ["20121221110926.38 "])
                .to_datetime()
                .unwrap(),
            DicomDateTime::from_date_and_time(
                DicomDate::from_ymd(2012, 12, 21).unwrap(),
                DicomTime::from_hmsf(11, 9, 26, 38, 2).unwrap()
            )
            .unwrap()
        );
        // not a dicom_datetime
        assert!(matches!(
            PrimitiveValue::from("Smith^John").to_datetime(),
            Err(ConvertValueError {
                requested: "DicomDateTime",
                original: ValueType::Str,
                ..
            })
        ));
    }

    #[test]
    fn primitive_value_to_multi_dicom_datetime() {
        // from text (Strs)
        assert_eq!(
            dicom_value!(
                Strs,
                ["20121221110926.38 ", "1992", "19901010-0500", "1990+0501"]
            )
            .to_multi_datetime()
            .unwrap(),
            vec!(
                DicomDateTime::from_date_and_time(
                    DicomDate::from_ymd(2012, 12, 21).unwrap(),
                    DicomTime::from_hmsf(11, 9, 26, 38, 2).unwrap()
                )
                .unwrap(),
                DicomDateTime::from_date(DicomDate::from_y(1992).unwrap()),
                DicomDateTime::from_date_with_time_zone(
                    DicomDate::from_ymd(1990, 10, 10).unwrap(),
                    FixedOffset::west_opt(5 * 3600).unwrap()
                ),
                DicomDateTime::from_date_with_time_zone(
                    DicomDate::from_y(1990).unwrap(),
                    FixedOffset::east_opt(5 * 3600 + 60).unwrap()
                )
            )
        );
    }

    #[test]
    fn primitive_value_to_date_range() {
        // converts first value of sequence
        assert_eq!(
            dicom_value!(Strs, ["20121221-", "1992-", "1990-1992", "1990+0501"])
                .to_date_range()
                .unwrap(),
            DateRange::from_start(NaiveDate::from_ymd_opt(2012, 12, 21).unwrap())
        );
    }

    #[test]
    fn primitive_value_to_time_range() {
        assert_eq!(
            dicom_value!(Str, "-153012.123").to_time_range().unwrap(),
            TimeRange::from_end(NaiveTime::from_hms_micro_opt(15, 30, 12, 123_999).unwrap())
        );
        assert_eq!(
            PrimitiveValue::from(&b"1015-"[..]).to_time_range().unwrap(),
            TimeRange::from_start(NaiveTime::from_hms_opt(10, 15, 0).unwrap())
        );
    }

    #[test]
    fn primitive_value_to_datetime_range() {
        assert_eq!(
            dicom_value!(Str, "202002-20210228153012.123")
                .to_datetime_range()
                .unwrap(),
            DateTimeRange::from_start_to_end(
                NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(2020, 2, 1).unwrap(),
                    NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                ),
                NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(2021, 2, 28).unwrap(),
                    NaiveTime::from_hms_micro_opt(15, 30, 12, 123_999).unwrap()
                )
            )
            .unwrap()
        );
        // East UTC offset gets parsed and the missing lower bound time-zone
        // will be the local clock time-zone offset
        assert_eq!(
            PrimitiveValue::from(&b"2020-2030+0800"[..])
                .to_datetime_range()
                .unwrap(),
            DateTimeRange::TimeZone {
                start: Some(
                    Local::now()
                        .offset()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                            NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                        ))
                        .unwrap()
                ),
                end: Some(
                    FixedOffset::east_opt(8 * 3600)
                        .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(2030, 12, 31).unwrap(),
                            NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
                        ))
                        .unwrap()
                )
            }
        );
    }
}
