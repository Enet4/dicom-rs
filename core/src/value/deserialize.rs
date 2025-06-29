//! Parsing of primitive values
use crate::value::partial::{
    check_component, DateComponent, DicomDate, DicomDateTime, DicomTime,
    Error as PartialValuesError,
};
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::convert::TryFrom;
use std::ops::{Add, Mul, Sub};

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Unexpected end of element"))]
    UnexpectedEndOfElement { backtrace: Backtrace },
    #[snafu(display("Invalid date"))]
    InvalidDate { backtrace: Backtrace },
    #[snafu(display("Invalid time"))]
    InvalidTime { backtrace: Backtrace },
    #[snafu(display("Invalid DateTime"))]
    InvalidDateTime {
        #[snafu(backtrace)]
        source: PartialValuesError,
    },
    #[snafu(display("Invalid date-time zone component"))]
    InvalidDateTimeZone { backtrace: Backtrace },
    #[snafu(display("Expected fraction delimiter '.', got '{}'", *value as char))]
    FractionDelimiter { value: u8, backtrace: Backtrace },
    #[snafu(display("Invalid number length: it is {}, but must be between 1 and 9", len))]
    InvalidNumberLength { len: usize, backtrace: Backtrace },
    #[snafu(display("Invalid number token: got '{}', but must be a digit in '0'..='9'", *value as char))]
    InvalidNumberToken { value: u8, backtrace: Backtrace },
    #[snafu(display("Invalid time zone sign token: got '{}', but must be '+' or '-'", *value as char))]
    InvalidTimeZoneSignToken { value: u8, backtrace: Backtrace },
    #[snafu(display(
        "Could not parse incomplete value: first missing component: {:?}",
        component
    ))]
    IncompleteValue {
        component: DateComponent,
        backtrace: Backtrace,
    },
    #[snafu(display("Component is invalid"))]
    InvalidComponent {
        #[snafu(backtrace)]
        source: PartialValuesError,
    },
    #[snafu(display("Failed to construct partial value"))]
    PartialValue {
        #[snafu(backtrace)]
        source: PartialValuesError,
    },
    #[snafu(display("Seconds '{secs}' out of bounds when constructing FixedOffset"))]
    SecsOutOfBounds { secs: i32, backtrace: Backtrace },
}

type Result<T, E = Error> = std::result::Result<T, E>;

/// Decode a single DICOM Date (DA) into a `chrono::NaiveDate` value.
/// As per standard, a full 8 byte representation (YYYYMMDD) is required,
/// otherwise, the operation fails.
pub fn parse_date(buf: &[u8]) -> Result<NaiveDate> {
    match buf.len() {
        4 => IncompleteValueSnafu {
            component: DateComponent::Month,
        }
        .fail(),
        6 => IncompleteValueSnafu {
            component: DateComponent::Day,
        }
        .fail(),
        len if len >= 8 => {
            let year = read_number(&buf[0..4])?;
            let month: u32 = read_number(&buf[4..6])?;
            check_component(DateComponent::Month, &month).context(InvalidComponentSnafu)?;

            let day: u32 = read_number(&buf[6..8])?;
            check_component(DateComponent::Day, &day).context(InvalidComponentSnafu)?;

            NaiveDate::from_ymd_opt(year, month, day).context(InvalidDateSnafu)
        }
        _ => UnexpectedEndOfElementSnafu.fail(),
    }
}

/** Decode a single DICOM Date (DA) into a `DicomDate` value.
 * Unlike `parse_date`, this method accepts incomplete dates such as YYYY and YYYYMM
 * The precision of the value is stored.
 */
pub fn parse_date_partial(buf: &[u8]) -> Result<(DicomDate, &[u8])> {
    if buf.len() < 4 {
        UnexpectedEndOfElementSnafu.fail()
    } else {
        let year: u16 = read_number(&buf[0..4])?;
        let buf = &buf[4..];
        if buf.len() < 2 {
            Ok((DicomDate::from_y(year).context(PartialValueSnafu)?, buf))
        } else {
            match read_number::<u8>(&buf[0..2]) {
                Err(_) => Ok((DicomDate::from_y(year).context(PartialValueSnafu)?, buf)),
                Ok(month) => {
                    let buf = &buf[2..];
                    if buf.len() < 2 {
                        Ok((
                            DicomDate::from_ym(year, month).context(PartialValueSnafu)?,
                            buf,
                        ))
                    } else {
                        match read_number::<u8>(&buf[0..2]) {
                            Err(_) => Ok((
                                DicomDate::from_ym(year, month).context(PartialValueSnafu)?,
                                buf,
                            )),
                            Ok(day) => {
                                let buf = &buf[2..];
                                Ok((
                                    DicomDate::from_ymd(year, month, day)
                                        .context(PartialValueSnafu)?,
                                    buf,
                                ))
                            }
                        }
                    }
                }
            }
        }
    }
}

/** Decode a single DICOM Time (TM) into a `DicomTime` value.
 * Unlike `parse_time`, this method allows for missing Time components.
 * The precision of the second fraction is stored and can be returned as a range later.
 */
pub fn parse_time_partial(buf: &[u8]) -> Result<(DicomTime, &[u8])> {
    if buf.len() < 2 {
        UnexpectedEndOfElementSnafu.fail()
    } else {
        let hour: u8 = read_number(&buf[0..2])?;
        let buf = &buf[2..];
        if buf.len() < 2 {
            Ok((DicomTime::from_h(hour).context(PartialValueSnafu)?, buf))
        } else {
            match read_number::<u8>(&buf[0..2]) {
                Err(_) => Ok((DicomTime::from_h(hour).context(PartialValueSnafu)?, buf)),
                Ok(minute) => {
                    let buf = &buf[2..];
                    if buf.len() < 2 {
                        Ok((
                            DicomTime::from_hm(hour, minute).context(PartialValueSnafu)?,
                            buf,
                        ))
                    } else {
                        match read_number::<u8>(&buf[0..2]) {
                            Err(_) => Ok((
                                DicomTime::from_hm(hour, minute).context(PartialValueSnafu)?,
                                buf,
                            )),
                            Ok(second) => {
                                let buf = &buf[2..];
                                // buf contains at least ".F" otherwise ignore
                                if buf.len() > 1 && buf[0] == b'.' {
                                    let buf = &buf[1..];
                                    let no_digits_index =
                                        buf.iter().position(|b| !b.is_ascii_digit());
                                    let max = no_digits_index.unwrap_or(buf.len());
                                    let n = usize::min(6, max);
                                    let fraction: u32 = read_number(&buf[0..n])?;
                                    let buf = &buf[n..];
                                    let fp = u8::try_from(n).unwrap();
                                    Ok((
                                        DicomTime::from_hmsf(hour, minute, second, fraction, fp)
                                            .context(PartialValueSnafu)?,
                                        buf,
                                    ))
                                } else {
                                    Ok((
                                        DicomTime::from_hms(hour, minute, second)
                                            .context(PartialValueSnafu)?,
                                        buf,
                                    ))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/** Decode a single DICOM Time (TM) into a `chrono::NaiveTime` value.
* If a time component is missing, the operation fails.
* Presence of the second fraction component `.FFFFFF` is mandatory with at
  least one digit accuracy `.F` while missing digits default to zero.
* For Time with missing components, or if exact second fraction accuracy needs to be preserved,
  use `parse_time_partial`.
*/
pub fn parse_time(buf: &[u8]) -> Result<(NaiveTime, &[u8])> {
    // at least HHMMSS.F required
    match buf.len() {
        2 => IncompleteValueSnafu {
            component: DateComponent::Minute,
        }
        .fail(),
        4 => IncompleteValueSnafu {
            component: DateComponent::Second,
        }
        .fail(),
        6 => {
            let hour: u32 = read_number(&buf[0..2])?;
            check_component(DateComponent::Hour, &hour).context(InvalidComponentSnafu)?;
            let minute: u32 = read_number(&buf[2..4])?;
            check_component(DateComponent::Minute, &minute).context(InvalidComponentSnafu)?;
            let second: u32 = read_number(&buf[4..6])?;
            check_component(DateComponent::Second, &second).context(InvalidComponentSnafu)?;
            Ok((
                NaiveTime::from_hms_opt(hour, minute, second).context(InvalidTimeSnafu)?,
                &buf[6..],
            ))
        }
        len if len >= 8 => {
            let hour: u32 = read_number(&buf[0..2])?;
            check_component(DateComponent::Hour, &hour).context(InvalidComponentSnafu)?;
            let minute: u32 = read_number(&buf[2..4])?;
            check_component(DateComponent::Minute, &minute).context(InvalidComponentSnafu)?;
            let second: u32 = read_number(&buf[4..6])?;
            check_component(DateComponent::Second, &second).context(InvalidComponentSnafu)?;
            let buf = &buf[6..];
            if buf[0] != b'.' {
                FractionDelimiterSnafu { value: buf[0] }.fail()
            } else {
                let buf = &buf[1..];
                let no_digits_index = buf.iter().position(|b| !b.is_ascii_digit());
                let max = no_digits_index.unwrap_or(buf.len());
                let n = usize::min(6, max);
                let mut fraction: u32 = read_number(&buf[0..n])?;
                let mut acc = n;
                while acc < 6 {
                    fraction *= 10;
                    acc += 1;
                }
                let buf = &buf[n..];
                check_component(DateComponent::Fraction, &fraction)
                    .context(InvalidComponentSnafu)?;
                Ok((
                    NaiveTime::from_hms_micro_opt(hour, minute, second, fraction)
                        .context(InvalidTimeSnafu)?,
                    buf,
                ))
            }
        }
        _ => UnexpectedEndOfElementSnafu.fail(),
    }
}

/// A simple trait for types with a decimal form.
pub trait Ten {
    /// Retrieve the value ten. This returns `10` for integer types and
    /// `10.` for floating point types.
    fn ten() -> Self;
}

macro_rules! impl_integral_ten {
    ($t:ty) => {
        impl Ten for $t {
            fn ten() -> Self {
                10
            }
        }
    };
}

macro_rules! impl_floating_ten {
    ($t:ty) => {
        impl Ten for $t {
            fn ten() -> Self {
                10.
            }
        }
    };
}

impl_integral_ten!(i16);
impl_integral_ten!(u16);
impl_integral_ten!(u8);
impl_integral_ten!(i32);
impl_integral_ten!(u32);
impl_integral_ten!(i64);
impl_integral_ten!(u64);
impl_integral_ten!(isize);
impl_integral_ten!(usize);
impl_floating_ten!(f32);
impl_floating_ten!(f64);

/// Retrieve an integer in text form.
///
/// All bytes in the text must be within the range b'0' and b'9'
/// The text must also not be empty nor have more than 9 characters.
pub fn read_number<T>(text: &[u8]) -> Result<T>
where
    T: Ten,
    T: From<u8>,
    T: Add<T, Output = T>,
    T: Mul<T, Output = T>,
    T: Sub<T, Output = T>,
{
    if text.is_empty() || text.len() > 9 {
        return InvalidNumberLengthSnafu { len: text.len() }.fail();
    }
    if let Some(c) = text.iter().cloned().find(|b| !b.is_ascii_digit()) {
        return InvalidNumberTokenSnafu { value: c }.fail();
    }

    Ok(read_number_unchecked(text))
}

#[inline]
fn read_number_unchecked<T>(buf: &[u8]) -> T
where
    T: Ten,
    T: From<u8>,
    T: Add<T, Output = T>,
    T: Mul<T, Output = T>,
{
    debug_assert!(!buf.is_empty());
    debug_assert!(buf.len() < 10);
    buf[1..].iter().fold((buf[0] - b'0').into(), |acc, v| {
        acc * T::ten() + (*v - b'0').into()
    })
}

/// Retrieve a `chrono::DateTime` from the given text, while assuming the given UTC offset.
///
/// If a date/time component is missing, the operation fails.
/// Presence of the second fraction component `.FFFFFF` is mandatory with at
/// least one digit accuracy `.F` while missing digits default to zero.
///
/// [`parse_datetime_partial`] should be preferred,
/// because it is more flexible and resilient to missing components.
/// See also the implementation of [`FromStr`](std::str::FromStr)
/// for [`DicomDateTime`].
#[deprecated(
    since = "0.7.0",
    note = "Use `parse_datetime_partial()` then `to_precise_datetime()`"
)]
pub fn parse_datetime(buf: &[u8], dt_utc_offset: FixedOffset) -> Result<DateTime<FixedOffset>> {
    let date = parse_date(buf)?;
    let buf = &buf[8..];
    let (time, buf) = parse_time(buf)?;
    let offset = match buf.len() {
        0 => {
            // A Date Time value without the optional suffix should be interpreted to be
            // the local time zone of the application creating the Data Element, and can
            // be overridden by the _Timezone Offset from UTC_ attribute.
            let dt: Result<_> = dt_utc_offset
                .from_local_datetime(&NaiveDateTime::new(date, time))
                .single()
                .context(InvalidDateTimeZoneSnafu);

            return dt;
        }
        len if len > 4 => {
            let tz_sign = buf[0];
            let buf = &buf[1..];
            let tz_h: i32 = read_number(&buf[0..2])?;
            let tz_m: i32 = read_number(&buf[2..4])?;
            let s = (tz_h * 60 + tz_m) * 60;
            match tz_sign {
                b'+' => FixedOffset::east_opt(s).context(SecsOutOfBoundsSnafu { secs: s })?,
                b'-' => FixedOffset::west_opt(s).context(SecsOutOfBoundsSnafu { secs: s })?,
                c => return InvalidTimeZoneSignTokenSnafu { value: c }.fail(),
            }
        }
        _ => return UnexpectedEndOfElementSnafu.fail(),
    };

    offset
        .from_local_datetime(&NaiveDateTime::new(date, time))
        .single()
        .context(InvalidDateTimeZoneSnafu)
}

/// Decode the text from the byte slice into a [`DicomDateTime`] value,
/// which allows for missing Date / Time components.
///
/// This is the underlying implementation of [`FromStr`](std::str::FromStr)
/// for `DicomDateTime`.
///
/// # Example
///
/// ```
/// # use dicom_core::value::deserialize::parse_datetime_partial;
/// use dicom_core::value::{DicomDate, DicomDateTime, DicomTime, PreciseDateTime};
/// use chrono::Datelike;
///
/// let input = "20240201123456.000305";
/// let dt = parse_datetime_partial(input.as_bytes())?;
/// assert_eq!(
///     dt,
///     DicomDateTime::from_date_and_time(
///         DicomDate::from_ymd(2024, 2, 1).unwrap(),
///         DicomTime::from_hms_micro(12, 34, 56, 305).unwrap(),
///     )?
/// );
/// // reinterpret as a chrono date time (with or without time zone)
/// let dt: PreciseDateTime = dt.to_precise_datetime()?;
/// // get just the date, for example
/// let date = dt.to_naive_date();
/// assert_eq!(date.year(), 2024);
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn parse_datetime_partial(buf: &[u8]) -> Result<DicomDateTime> {
    let (date, rest) = parse_date_partial(buf)?;

    let (time, buf) = match parse_time_partial(rest) {
        Ok((time, buf)) => (Some(time), buf),
        Err(_) => (None, rest),
    };

    let time_zone = match buf.len() {
        0 => None,
        len if len > 4 => {
            let tz_sign = buf[0];
            let buf = &buf[1..];
            let tz_h: u32 = read_number(&buf[0..2])?;
            let tz_m: u32 = read_number(&buf[2..4])?;
            let s = (tz_h * 60 + tz_m) * 60;
            match tz_sign {
                b'+' => {
                    check_component(DateComponent::UtcEast, &s).context(InvalidComponentSnafu)?;
                    Some(
                        FixedOffset::east_opt(s as i32)
                            .context(SecsOutOfBoundsSnafu { secs: s as i32 })?,
                    )
                }
                b'-' => {
                    check_component(DateComponent::UtcWest, &s).context(InvalidComponentSnafu)?;
                    Some(
                        FixedOffset::west_opt(s as i32)
                            .context(SecsOutOfBoundsSnafu { secs: s as i32 })?,
                    )
                }
                c => return InvalidTimeZoneSignTokenSnafu { value: c }.fail(),
            }
        }
        _ => return UnexpectedEndOfElementSnafu.fail(),
    };

    match time_zone {
        Some(time_zone) => match time {
            Some(tm) => DicomDateTime::from_date_and_time_with_time_zone(date, tm, time_zone)
                .context(InvalidDateTimeSnafu),
            None => Ok(DicomDateTime::from_date_with_time_zone(date, time_zone)),
        },
        None => match time {
            Some(tm) => DicomDateTime::from_date_and_time(date, tm).context(InvalidDateTimeSnafu),
            None => Ok(DicomDateTime::from_date(date)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date() {
        assert_eq!(
            parse_date(b"20180101").unwrap(),
            NaiveDate::from_ymd_opt(2018, 1, 1).unwrap()
        );
        assert_eq!(
            parse_date(b"19711231").unwrap(),
            NaiveDate::from_ymd_opt(1971, 12, 31).unwrap()
        );
        assert_eq!(
            parse_date(b"20140426").unwrap(),
            NaiveDate::from_ymd_opt(2014, 4, 26).unwrap()
        );
        assert_eq!(
            parse_date(b"20180101xxxx").unwrap(),
            NaiveDate::from_ymd_opt(2018, 1, 1).unwrap()
        );
        assert_eq!(
            parse_date(b"19000101").unwrap(),
            NaiveDate::from_ymd_opt(1900, 1, 1).unwrap()
        );
        assert_eq!(
            parse_date(b"19620728").unwrap(),
            NaiveDate::from_ymd_opt(1962, 7, 28).unwrap()
        );
        assert_eq!(
            parse_date(b"19020404-0101").unwrap(),
            NaiveDate::from_ymd_opt(1902, 4, 4).unwrap()
        );

        assert!(matches!(
            parse_date(b"1902"),
            Err(Error::IncompleteValue {
                component: DateComponent::Month,
                ..
            })
        ));

        assert!(matches!(
            parse_date(b"190208"),
            Err(Error::IncompleteValue {
                component: DateComponent::Day,
                ..
            })
        ));

        assert!(matches!(
            parse_date(b"19021515"),
            Err(Error::InvalidComponent {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Month,
                    value: 15,
                    ..
                },
                ..
            })
        ));

        assert!(matches!(
            parse_date(b"19021200"),
            Err(Error::InvalidComponent {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Day,
                    value: 0,
                    ..
                },
                ..
            })
        ));

        assert!(matches!(
            parse_date(b"19021232"),
            Err(Error::InvalidComponent {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Day,
                    value: 32,
                    ..
                },
                ..
            })
        ));

        // not a leap year
        assert!(matches!(
            parse_date(b"20210229"),
            Err(Error::InvalidDate { .. })
        ));

        assert!(parse_date(b"").is_err());
        assert!(parse_date(b"        ").is_err());
        assert!(parse_date(b"--------").is_err());
        assert!(parse_date(&[0x00_u8; 8]).is_err());
        assert!(parse_date(&[0xFF_u8; 8]).is_err());
        assert!(parse_date(&[b'0'; 8]).is_err());
        assert!(parse_date(b"nothing!").is_err());
        assert!(parse_date(b"2012dec").is_err());
    }

    #[test]
    fn test_parse_date_partial() {
        assert_eq!(
            parse_date_partial(b"20180101").unwrap(),
            (DicomDate::from_ymd(2018, 1, 1).unwrap(), &[][..])
        );
        assert_eq!(
            parse_date_partial(b"19711231").unwrap(),
            (DicomDate::from_ymd(1971, 12, 31).unwrap(), &[][..])
        );
        assert_eq!(
            parse_date_partial(b"20180101xxxx").unwrap(),
            (DicomDate::from_ymd(2018, 1, 1).unwrap(), &b"xxxx"[..])
        );
        assert_eq!(
            parse_date_partial(b"201801xxxx").unwrap(),
            (DicomDate::from_ym(2018, 1).unwrap(), &b"xxxx"[..])
        );
        assert_eq!(
            parse_date_partial(b"2018xxxx").unwrap(),
            (DicomDate::from_y(2018).unwrap(), &b"xxxx"[..])
        );
        assert_eq!(
            parse_date_partial(b"19020404-0101").unwrap(),
            (DicomDate::from_ymd(1902, 4, 4).unwrap(), &b"-0101"[..][..])
        );
        assert_eq!(
            parse_date_partial(b"201811").unwrap(),
            (DicomDate::from_ym(2018, 11).unwrap(), &[][..])
        );
        assert_eq!(
            parse_date_partial(b"1914").unwrap(),
            (DicomDate::from_y(1914).unwrap(), &[][..])
        );

        assert_eq!(
            parse_date_partial(b"19140").unwrap(),
            (DicomDate::from_y(1914).unwrap(), &b"0"[..])
        );

        assert_eq!(
            parse_date_partial(b"1914121").unwrap(),
            (DicomDate::from_ym(1914, 12).unwrap(), &b"1"[..])
        );

        // does not check for leap year
        assert_eq!(
            parse_date_partial(b"20210229").unwrap(),
            (DicomDate::from_ymd(2021, 2, 29).unwrap(), &[][..])
        );

        assert!(matches!(
            parse_date_partial(b"19021515"),
            Err(Error::PartialValue {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Month,
                    value: 15,
                    ..
                },
                ..
            })
        ));

        assert!(matches!(
            parse_date_partial(b"19021200"),
            Err(Error::PartialValue {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Day,
                    value: 0,
                    ..
                },
                ..
            })
        ));

        assert!(matches!(
            parse_date_partial(b"19021232"),
            Err(Error::PartialValue {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Day,
                    value: 32,
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn test_parse_time() {
        assert_eq!(
            parse_time(b"100000.1").unwrap(),
            (
                NaiveTime::from_hms_micro_opt(10, 0, 0, 100_000).unwrap(),
                &[][..]
            )
        );
        assert_eq!(
            parse_time(b"235959.0123").unwrap(),
            (
                NaiveTime::from_hms_micro_opt(23, 59, 59, 12_300).unwrap(),
                &[][..]
            )
        );
        // only parses 6 digit precision as in DICOM standard
        assert_eq!(
            parse_time(b"235959.1234567").unwrap(),
            (
                NaiveTime::from_hms_micro_opt(23, 59, 59, 123_456).unwrap(),
                &b"7"[..]
            )
        );
        assert_eq!(
            parse_time(b"235959.123456+0100").unwrap(),
            (
                NaiveTime::from_hms_micro_opt(23, 59, 59, 123_456).unwrap(),
                &b"+0100"[..]
            )
        );
        assert_eq!(
            parse_time(b"235959.1-0100").unwrap(),
            (
                NaiveTime::from_hms_micro_opt(23, 59, 59, 100_000).unwrap(),
                &b"-0100"[..]
            )
        );
        assert_eq!(
            parse_time(b"235959.12345+0100").unwrap(),
            (
                NaiveTime::from_hms_micro_opt(23, 59, 59, 123_450).unwrap(),
                &b"+0100"[..]
            )
        );
        assert_eq!(
            parse_time(b"153011").unwrap(),
            (NaiveTime::from_hms_opt(15, 30, 11).unwrap(), &b""[..])
        );
        assert_eq!(
            parse_time(b"000000.000000").unwrap(),
            (NaiveTime::from_hms_opt(0, 0, 0).unwrap(), &[][..])
        );
        assert!(matches!(
            parse_time(b"23"),
            Err(Error::IncompleteValue {
                component: DateComponent::Minute,
                ..
            })
        ));
        assert!(matches!(
            parse_time(b"1530"),
            Err(Error::IncompleteValue {
                component: DateComponent::Second,
                ..
            })
        ));
        assert!(matches!(
            parse_time(b"153011x0110"),
            Err(Error::FractionDelimiter { value: 0x78_u8, .. })
        ));
        assert!(parse_date(&[0x00_u8; 6]).is_err());
        assert!(parse_date(&[0xFF_u8; 6]).is_err());
        assert!(parse_date(b"075501.----").is_err());
        assert!(parse_date(b"nope").is_err());
        assert!(parse_date(b"235800.0a").is_err());
    }
    #[test]
    fn test_parse_time_partial() {
        assert_eq!(
            parse_time_partial(b"10").unwrap(),
            (DicomTime::from_h(10).unwrap(), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"101").unwrap(),
            (DicomTime::from_h(10).unwrap(), &b"1"[..])
        );
        assert_eq!(
            parse_time_partial(b"0755").unwrap(),
            (DicomTime::from_hm(7, 55).unwrap(), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075500").unwrap(),
            (DicomTime::from_hms(7, 55, 0).unwrap(), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"065003").unwrap(),
            (DicomTime::from_hms(6, 50, 3).unwrap(), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075501.5").unwrap(),
            (DicomTime::from_hmsf(7, 55, 1, 5, 1).unwrap(), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075501.123").unwrap(),
            (DicomTime::from_hmsf(7, 55, 1, 123, 3).unwrap(), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"10+0101").unwrap(),
            (DicomTime::from_h(10).unwrap(), &b"+0101"[..])
        );
        assert_eq!(
            parse_time_partial(b"1030+0101").unwrap(),
            (DicomTime::from_hm(10, 30).unwrap(), &b"+0101"[..])
        );
        assert_eq!(
            parse_time_partial(b"075501.123+0101").unwrap(),
            (
                DicomTime::from_hmsf(7, 55, 1, 123, 3).unwrap(),
                &b"+0101"[..]
            )
        );
        assert_eq!(
            parse_time_partial(b"075501+0101").unwrap(),
            (DicomTime::from_hms(7, 55, 1).unwrap(), &b"+0101"[..])
        );
        assert_eq!(
            parse_time_partial(b"075501.999999").unwrap(),
            (DicomTime::from_hmsf(7, 55, 1, 999_999, 6).unwrap(), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075501.9999994").unwrap(),
            (
                DicomTime::from_hmsf(7, 55, 1, 999_999, 6).unwrap(),
                &b"4"[..]
            )
        );
        // 60 seconds for leap second
        assert_eq!(
            parse_time_partial(b"105960").unwrap(),
            (DicomTime::from_hms(10, 59, 60).unwrap(), &[][..])
        );
        assert!(matches!(
            parse_time_partial(b"24"),
            Err(Error::PartialValue {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Hour,
                    value: 24,
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            parse_time_partial(b"1060"),
            Err(Error::PartialValue {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Minute,
                    value: 60,
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn test_parse_datetime_partial() {
        assert_eq!(
            parse_datetime_partial(b"20171130101010.204").unwrap(),
            DicomDateTime::from_date_and_time(
                DicomDate::from_ymd(2017, 11, 30).unwrap(),
                DicomTime::from_hmsf(10, 10, 10, 204, 3).unwrap(),
            )
            .unwrap()
        );
        assert_eq!(
            parse_datetime_partial(b"20171130101010").unwrap(),
            DicomDateTime::from_date_and_time(
                DicomDate::from_ymd(2017, 11, 30).unwrap(),
                DicomTime::from_hms(10, 10, 10).unwrap()
            )
            .unwrap()
        );
        assert_eq!(
            parse_datetime_partial(b"2017113023").unwrap(),
            DicomDateTime::from_date_and_time(
                DicomDate::from_ymd(2017, 11, 30).unwrap(),
                DicomTime::from_h(23).unwrap()
            )
            .unwrap()
        );
        assert_eq!(
            parse_datetime_partial(b"201711").unwrap(),
            DicomDateTime::from_date(DicomDate::from_ym(2017, 11).unwrap())
        );
        assert_eq!(
            parse_datetime_partial(b"20171130101010.204+0535").unwrap(),
            DicomDateTime::from_date_and_time_with_time_zone(
                DicomDate::from_ymd(2017, 11, 30).unwrap(),
                DicomTime::from_hmsf(10, 10, 10, 204, 3).unwrap(),
                FixedOffset::east_opt(5 * 3600 + 35 * 60).unwrap()
            )
            .unwrap()
        );
        assert_eq!(
            parse_datetime_partial(b"20171130101010+0535").unwrap(),
            DicomDateTime::from_date_and_time_with_time_zone(
                DicomDate::from_ymd(2017, 11, 30).unwrap(),
                DicomTime::from_hms(10, 10, 10).unwrap(),
                FixedOffset::east_opt(5 * 3600 + 35 * 60).unwrap()
            )
            .unwrap()
        );
        assert_eq!(
            parse_datetime_partial(b"2017113010+0535").unwrap(),
            DicomDateTime::from_date_and_time_with_time_zone(
                DicomDate::from_ymd(2017, 11, 30).unwrap(),
                DicomTime::from_h(10).unwrap(),
                FixedOffset::east_opt(5 * 3600 + 35 * 60).unwrap()
            )
            .unwrap()
        );
        assert_eq!(
            parse_datetime_partial(b"20171130-0135").unwrap(),
            DicomDateTime::from_date_with_time_zone(
                DicomDate::from_ymd(2017, 11, 30).unwrap(),
                FixedOffset::west_opt(3600 + 35 * 60).unwrap()
            )
        );
        assert_eq!(
            parse_datetime_partial(b"201711-0135").unwrap(),
            DicomDateTime::from_date_with_time_zone(
                DicomDate::from_ym(2017, 11).unwrap(),
                FixedOffset::west_opt(3600 + 35 * 60).unwrap()
            )
        );
        assert_eq!(
            parse_datetime_partial(b"2017-0135").unwrap(),
            DicomDateTime::from_date_with_time_zone(
                DicomDate::from_y(2017).unwrap(),
                FixedOffset::west_opt(3600 + 35 * 60).unwrap()
            )
        );

        // West UTC offset out of range
        assert!(matches!(
            parse_datetime_partial(b"20200101-1201"),
            Err(Error::InvalidComponent { .. })
        ));

        // East UTC offset out of range
        assert!(matches!(
            parse_datetime_partial(b"20200101+1401"),
            Err(Error::InvalidComponent { .. })
        ));

        assert!(matches!(
            parse_datetime_partial(b"xxxx0229101010.204"),
            Err(Error::InvalidNumberToken { .. })
        ));

        assert!(parse_datetime_partial(b"").is_err());
        assert!(parse_datetime_partial(&[0x00_u8; 8]).is_err());
        assert!(parse_datetime_partial(&[0xFF_u8; 8]).is_err());
        assert!(parse_datetime_partial(&[b'0'; 8]).is_err());
        assert!(parse_datetime_partial(&[b' '; 8]).is_err());
        assert!(parse_datetime_partial(b"nope").is_err());
        assert!(parse_datetime_partial(b"2015dec").is_err());
        assert!(parse_datetime_partial(b"20151231162945.").is_err());
        assert!(parse_datetime_partial(b"20151130161445+").is_err());
        assert!(parse_datetime_partial(b"20151130161445+----").is_err());
        assert!(parse_datetime_partial(b"20151130161445. ").is_err());
        assert!(parse_datetime_partial(b"20151130161445. +0000").is_err());
        assert!(parse_datetime_partial(b"20100423164000.001+3").is_err());
        assert!(parse_datetime_partial(b"200809112945*1000").is_err());
        assert!(parse_datetime_partial(b"20171130101010.204+1").is_err());
        assert!(parse_datetime_partial(b"20171130101010.204+01").is_err());
        assert!(parse_datetime_partial(b"20171130101010.204+011").is_err());
    }
}
