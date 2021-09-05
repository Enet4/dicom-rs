//! Parsing of primitive values
use crate::value::partial::{
    check_component, AsTemporalRange, DateComponent, Error as PartialValuesError, PartialDate,
    PartialTime,
};
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
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
    /// TODO DELETE THIS UP TO ...
    #[snafu(display("Invalid month component: got {}, but must be in 1..=12", value))]
    InvalidDateMonth { value: u32, backtrace: Backtrace },
    #[snafu(display("Invalid day component: got {}, but must be in 1..=31", value))]
    InvalidDateDay { value: u32, backtrace: Backtrace },
    #[snafu(display("Invalid date-time zone component"))]
    InvalidDateTimeZone { backtrace: Backtrace },
    #[snafu(display("Invalid hour component: got {}, but must be in 0..24", value))]
    InvalidDateTimeHour { value: u32, backtrace: Backtrace },
    #[snafu(display("Invalid minute component: got {}, but must be in 0..60", value))]
    InvalidDateTimeMinute { value: u32, backtrace: Backtrace },
    #[snafu(display("Invalid second component: got {}, but must be in 0..60", value))]
    InvalidDateTimeSecond { value: u32, backtrace: Backtrace },
    #[snafu(display(
        "Invalid microsecond component: got {}, but must be in 0..2_000_000",
        value
    ))]
    InvalidDateTimeMicrosecond { value: u32, backtrace: Backtrace },

    #[snafu(display("Unexpected token after date: got '{}', but must be '.', '+', or '-'", *value as char))]
    UnexpectedAfterDateToken { value: u8, backtrace: Backtrace },
    // UP TO These ...
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
    #[snafu(display("Range is zero"))]
    ZeroRange { backtrace: Backtrace },
    #[snafu(display("Component is invalid."))]
    InvalidComponent {
        #[snafu(backtrace)]
        source: PartialValuesError,
    },
    #[snafu(display("Failed to construct partial value."))]
    PartialValue {
        #[snafu(backtrace)]
        source: PartialValuesError,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

/** Decode a single DICOM Date (DA) into a `NaiveDate` value.
  * As per standard, a full 8 byte representation (YYYYMMDD) is required,
  otherwise, the operation fails.
*/
pub fn parse_date(buf: &[u8]) -> Result<NaiveDate> {
    match buf.len() {
        4 => {
            let _year: i32 = read_number(&buf[0..4])?;
            IncompleteValue {
                component: DateComponent::Month,
            }
            .fail()
        }
        6 => {
            let _year: i32 = read_number(&buf[0..4])?;
            let month: u32 = read_number(&buf[4..6])?;
            check_component(DateComponent::Month, &month).context(InvalidComponent)?;
            IncompleteValue {
                component: DateComponent::Day,
            }
            .fail()
        }
        len if len >= 8 => {
            let year = read_number(&buf[0..4])?;
            let month: u32 = read_number(&buf[4..6])?;
            check_component(DateComponent::Month, &month).context(InvalidComponent)?;

            let day: u32 = read_number(&buf[6..8])?;
            check_component(DateComponent::Day, &day).context(InvalidComponent)?;

            NaiveDate::from_ymd_opt(year, month, day).context(InvalidDate)
        }
        _ => UnexpectedEndOfElement.fail(),
    }
}

/** Decode a single DICOM Date (DA) into a `PartialDate` value.
 * Unlike `parse_date`, this method accepts incomplete dates such as YYYY and YYYYMM
 * The precision of the value is stored.
 */
pub fn parse_date_partial(buf: &[u8]) -> Result<(PartialDate, &[u8])> {
    if buf.len() < 4 {
        UnexpectedEndOfElement.fail()
    } else {
        let year: u16 = read_number(&buf[0..4])?;
        let buf = &buf[4..];
        if buf.len() < 2 {
            Ok((PartialDate::from_y(&year).context(PartialValue)?, buf))
        } else {
            let month: u8 = read_number(&buf[0..2])?;
            let buf = &buf[2..];
            if buf.len() < 2 {
                Ok((
                    PartialDate::from_ym(&year, &month).context(PartialValue)?,
                    buf,
                ))
            } else {
                let day: u8 = read_number(&buf[0..2])?;
                let buf = &buf[2..];
                Ok((
                    PartialDate::from_ymd(&year, &month, &day).context(PartialValue)?,
                    buf,
                ))
            }
        }
    }
}

/** Decode a single DICOM Time (TM) into a `PartialTime` value.
 * Unlike `parse_time`, this method allows for missing Time components.
 * The precision of the second fraction is stored and can be returned as a range later.
 * b".123" is stored as 123, unlike 123_000 in `parse_time`
 */
pub fn parse_time_partial(buf: &[u8]) -> Result<(PartialTime, &[u8])> {
    if buf.len() < 2 {
        UnexpectedEndOfElement.fail()
    } else {
        let hour: u8 = read_number(&buf[0..2])?;
        let buf = &buf[2..];
        if buf.len() < 2 {
            Ok((PartialTime::from_h(&hour).context(PartialValue)?, buf))
        } else {
            let minute: u8 = read_number(&buf[0..2])?;
            let buf = &buf[2..];
            if buf.len() < 2 {
                Ok((
                    PartialTime::from_hm(&hour, &minute).context(PartialValue)?,
                    buf,
                ))
            } else {
                let second: u8 = read_number(&buf[0..2])?;
                let buf = &buf[2..];
                if buf.len() < 2 {
                    Ok((
                        PartialTime::from_hms(&hour, &minute, &second).context(PartialValue)?,
                        buf,
                    ))
                } else if buf[0] != b'.' {
                    FractionDelimiter { value: buf[0] }.fail()
                } else {
                    let buf = &buf[1..];
                    let n = usize::min(6, buf.len());
                    let fraction: u32 = read_number(&buf[0..n])?;
                    /*let mut acc = n;
                    while acc < 6 {
                        fraction *= 10;
                        acc += 1;
                    }*/
                    let buf = &buf[n..];
                    let fp = u8::try_from(n).unwrap();
                    Ok((
                        PartialTime::from_hmsf(&hour, &minute, &second, &fraction, &fp)
                            .context(PartialValue)?,
                        buf,
                    ))
                }
            }
        }
    }
}

/** Decode a single DICOM Time (TM) into a `NaiveTime` value.
* If a time component is missing, the operation fails.
* Presence of the second fraction component `.FFFFFF` is mandatory with at
  least one digit accuracy `.F` while missing digits default to zero.
* For Time with missing components, or if exact second fraction accuracy needs to be preserved,
  use `parse_time_partial`.
*/
pub fn parse_time(buf: &[u8]) -> Result<(NaiveTime, &[u8])> {
    // at least HHMMSS.F required
    match buf.len() {
        2 => {
            let hour: u32 = read_number(&buf[0..2])?;
            check_component(DateComponent::Hour, &hour).context(InvalidComponent)?;
            IncompleteValue {
                component: DateComponent::Minute,
            }
            .fail()
        }
        4 => {
            let hour: u32 = read_number(&buf[0..2])?;
            check_component(DateComponent::Hour, &hour).context(InvalidComponent)?;
            let minute: u32 = read_number(&buf[2..4])?;
            check_component(DateComponent::Minute, &minute).context(InvalidComponent)?;
            IncompleteValue {
                component: DateComponent::Second,
            }
            .fail()
        }
        6 => {
            let hour: u32 = read_number(&buf[0..2])?;
            check_component(DateComponent::Hour, &hour).context(InvalidComponent)?;
            let minute: u32 = read_number(&buf[2..4])?;
            check_component(DateComponent::Minute, &minute).context(InvalidComponent)?;
            let second: u32 = read_number(&buf[4..6])?;
            check_component(DateComponent::Second, &second).context(InvalidComponent)?;
            IncompleteValue {
                component: DateComponent::Fraction,
            }
            .fail()
        }
        len if len >= 8 => {
            let hour: u32 = read_number(&buf[0..2])?;
            check_component(DateComponent::Hour, &hour).context(InvalidComponent)?;
            let minute: u32 = read_number(&buf[2..4])?;
            check_component(DateComponent::Minute, &minute).context(InvalidComponent)?;
            let second: u32 = read_number(&buf[4..6])?;
            check_component(DateComponent::Second, &second).context(InvalidComponent)?;
            let buf = &buf[6..];
            if buf[0] != b'.' {
                FractionDelimiter { value: buf[0] }.fail()
            } else {
                let buf = &buf[1..];
                let no_digits_index = buf.iter().position(|b| !(b'0'..=b'9').contains(b));
                let max = no_digits_index.unwrap_or(buf.len());
                let n = usize::min(6, max);
                let mut fraction: u32 = read_number(&buf[0..n])?;
                let mut acc = n;
                while acc < 6 {
                    fraction *= 10;
                    acc += 1;
                }
                let buf = &buf[n..];
                check_component(DateComponent::Fraction, &fraction).context(InvalidComponent)?;
                Ok((
                    NaiveTime::from_hms_micro(hour, minute, second, fraction),
                    buf,
                ))
            }
        }
        _ => UnexpectedEndOfElement.fail(),
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
        return InvalidNumberLength { len: text.len() }.fail();
    }
    if let Some(c) = text.iter().cloned().find(|b| !(b'0'..=b'9').contains(b)) {
        return InvalidNumberToken { value: c }.fail();
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
    (&buf[1..]).iter().fold((buf[0] - b'0').into(), |acc, v| {
        acc * T::ten() + (*v - b'0').into()
    })
}

/** Retrieve a DICOM date-time from the given text, while assuming the given UTC offset.
* If a date/time component is missing, the operation fails.
* Presence of the second fraction component `.FFFFFF` is mandatory with at
  least one digit accuracy `.F` while missing digits default to zero.
* For DateTime with missing components, or if exact second fraction accuracy needs to be preserved,
  use `parse_datetime_partial`.
*/
pub fn parse_datetime(buf: &[u8], dt_utc_offset: FixedOffset) -> Result<DateTime<FixedOffset>> {
    let date = parse_date(buf)?;
    // date checks OK, so can skip exactly 8 bytes
    let buf = &buf[8..];
    let (time, buf) = parse_time(buf)?;
    let offset = match buf.len() {
        0 => {
            // A Date Time value without the optional suffix should be interpreted to be
            // the local time zone of the application creating the Data Element, and can
            // be overridden by the _Timezone Offset from UTC_ attribute.
            let dt: Result<_> = dt_utc_offset
                .from_local_date(&date)
                .and_time(time)
                .single()
                .context(InvalidDateTimeZone);
            return Ok(dt?);
        }
        len if len > 4 => {
            let tz_sign = buf[0];
            let buf = &buf[1..];
            let tz_h: i32 = read_number(&buf[0..2])?;
            let tz_m: i32 = read_number(&buf[2..4])?;
            let s = (tz_h * 60 + tz_m) * 60;
            match tz_sign {
                b'+' => FixedOffset::east(s),
                b'-' => FixedOffset::west(s),
                c => return InvalidTimeZoneSignToken { value: c }.fail(),
            }
        }
        _ => return UnexpectedEndOfElement.fail(),
    };

    offset
        .from_utc_date(&date)
        .and_time(time)
        .context(InvalidDateTimeZone)
}

/** Decode a single DICOM DateTime (DT) into a `PartialDateTime` value.
 * Unlike `parse_datetime`, this method allows for missing Date / Time components.
 * The precision of the second fraction is stored and can be returned as a range later.
 *//*
pub fn parse_datetime_partial(buf: &[u8], dt_utc_offset: FixedOffset) -> Result<DateTime<FixedOffset>> {
    let (date, rest) = parse_date_partial(buf)?;
    let (time, buf) = parse_time_partial(rest)?;

    let da = match date.precision() {
        DateComponent::Day => NaiveDate::from_ymd_opt(
            year: i32, month: u32, day: u32).context(InvalidDate),
            _ => {}
    };

    let offset = match buf.len() {
        0 => {
            // A Date Time value without the optional suffix should be interpreted to be
            // the local time zone of the application creating the Data Element, and can
            // be overridden by the _Timezone Offset from UTC_ attribute.
            let dt: Result<_> = dt_utc_offset
                .from_local_date(&date)
                .and_time(time)
                .single()
                .context(InvalidDateTimeZone);
            return Ok(dt?);
        }
        len if len > 4 => {
            let tz_sign = buf[0];
            let buf = &buf[1..];
            let tz_h: i32 = read_number(&buf[0..2])?;
            let tz_m: i32 = read_number(&buf[2..4])?;
            let s = (tz_h * 60 + tz_m) * 60;
            match tz_sign {
                b'+' => FixedOffset::east(s),
                b'-' => FixedOffset::west(s),
                c => return InvalidTimeZoneSignToken { value: c }.fail(),
            }
        }
        _ => return UnexpectedEndOfElement.fail(),
    };

    offset
        .from_utc_date(&date)
        .and_time(time)
        .context(InvalidDateTimeZone)
}*/


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, NaiveDate, NaiveTime, TimeZone};

    #[test]
    fn test_parse_date() {
        assert_eq!(
            parse_date(b"20180101").unwrap(),
            NaiveDate::from_ymd(2018, 1, 1)
        );
        assert_eq!(
            parse_date(b"19711231").unwrap(),
            NaiveDate::from_ymd(1971, 12, 31)
        );
        assert_eq!(
            parse_date(b"20140426").unwrap(),
            NaiveDate::from_ymd(2014, 4, 26)
        );
        assert_eq!(
            parse_date(b"20180101xxxx").unwrap(),
            NaiveDate::from_ymd(2018, 1, 1)
        );
        assert_eq!(
            parse_date(b"19000101").unwrap(),
            NaiveDate::from_ymd(1900, 1, 1)
        );
        assert_eq!(
            parse_date(b"19620728").unwrap(),
            NaiveDate::from_ymd(1962, 7, 28)
        );
        assert_eq!(
            parse_date(b"19020404-0101").unwrap(),
            NaiveDate::from_ymd(1902, 4, 4)
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
            (PartialDate::Day(2018, 1, 1), &[][..])
        );
        assert_eq!(
            parse_date_partial(b"19711231").unwrap(),
            (PartialDate::Day(1971, 12, 31), &[][..])
        );
        assert_eq!(
            parse_date_partial(b"20180101xxxx").unwrap(),
            (PartialDate::Day(2018, 1, 1), &b"xxxx"[..])
        );
        assert_eq!(
            parse_date_partial(b"19020404-0101").unwrap(),
            (PartialDate::Day(1902, 4, 4), &b"-0101"[..][..])
        );
        assert_eq!(
            parse_date_partial(b"201811").unwrap(),
            (PartialDate::Month(2018, 11), &[][..])
        );
        assert_eq!(
            parse_date_partial(b"1914").unwrap(),
            (PartialDate::Year(1914), &[][..])
        );

        assert_eq!(
            parse_date_partial(b"19140").unwrap(),
            (PartialDate::Year(1914), &b"0"[..])
        );

        assert_eq!(
            parse_date_partial(b"1914121").unwrap(),
            (PartialDate::Month(1914, 12), &b"1"[..])
        );

        // does not check for leap year
        assert_eq!(
            parse_date_partial(b"20210229").unwrap(),
            (PartialDate::Day(2021, 2, 29), &[][..])
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
            (NaiveTime::from_hms_micro(10, 0, 0, 100_000), &[][..])
        );
        assert_eq!(
            parse_time(b"235959.0123").unwrap(),
            (NaiveTime::from_hms_micro(23, 59, 59, 12_300), &[][..])
        );
        // only parses 6 digit precision as in DICOM standard
        assert_eq!(
            parse_time(b"235959.1234567").unwrap(),
            (NaiveTime::from_hms_micro(23, 59, 59, 123_456), &b"7"[..])
        );
        assert_eq!(
            parse_time(b"235959.123456+0100").unwrap(),
            (
                NaiveTime::from_hms_micro(23, 59, 59, 123_456),
                &b"+0100"[..]
            )
        );
        assert_eq!(
            parse_time(b"235959.1-0100").unwrap(),
            (
                NaiveTime::from_hms_micro(23, 59, 59, 100_000),
                &b"-0100"[..]
            )
        );
        assert_eq!(
            parse_time(b"235959.12345+0100").unwrap(),
            (
                NaiveTime::from_hms_micro(23, 59, 59, 123_450),
                &b"+0100"[..]
            )
        );
        assert_eq!(
            parse_time(b"000000.000000").unwrap(),
            (NaiveTime::from_hms(0, 0, 0), &[][..])
        );
        assert!(matches!(
            parse_time(b"24"),
            Err(Error::InvalidComponent {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Hour,
                    value: 24,
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            parse_time(b"23"),
            Err(Error::IncompleteValue {
                component: DateComponent::Minute,
                ..
            })
        ));
        assert!(matches!(
            parse_time(b"1560"),
            Err(Error::InvalidComponent {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Minute,
                    value: 60,
                    ..
                },
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
            parse_time(b"153099"),
            Err(Error::InvalidComponent {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Second,
                    value: 99,
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            parse_time(b"153011"),
            Err(Error::IncompleteValue {
                component: DateComponent::Fraction,
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
            (PartialTime::Hour(10), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"101").unwrap(),
            (PartialTime::Hour(10), &b"1"[..])
        );
        assert_eq!(
            parse_time_partial(b"0755").unwrap(),
            (PartialTime::Minute(7, 55), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075500").unwrap(),
            (PartialTime::Second(7, 55, 0), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"065003").unwrap(),
            (PartialTime::Second(6, 50, 3), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075501.5").unwrap(),
            (PartialTime::Fraction(7, 55, 1, 5, 1), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075501.123").unwrap(),
            (PartialTime::Fraction(7, 55, 1, 123, 3), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075501.999999").unwrap(),
            (PartialTime::Fraction(7, 55, 1, 999_999, 6), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075501.9999994").unwrap(),
            (PartialTime::Fraction(7, 55, 1, 999_999, 6), &b"4"[..])
        );
        assert!(matches!(
            parse_time_partial(b"075501x123456"),
            Err(Error::FractionDelimiter { value: 0x78_u8, .. })
        ));
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
        assert!(matches!(
            parse_time_partial(b"105960"),
            Err(Error::PartialValue {
                source: PartialValuesError::InvalidComponent {
                    component: DateComponent::Second,
                    value: 60,
                    ..
                },
                ..
            })
        ));
    }
    #[test]
    fn test_datetime() {
        let default_offset = FixedOffset::east(0);
        assert_eq!(
            parse_datetime(b"20171130101010.204", default_offset).unwrap(),
            FixedOffset::east(0)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        assert_eq!(
            parse_datetime(b"19440229101010.1", default_offset).unwrap(),
            FixedOffset::east(0)
                .ymd(1944, 2, 29)
                .and_hms_micro(10, 10, 10, 100_000)
        );
        assert_eq!(
            parse_datetime(b"19450228101010.999999", default_offset).unwrap(),
            FixedOffset::east(0)
                .ymd(1945, 2, 28)
                .and_hms_micro(10, 10, 10, 999_999)
        );
        assert_eq!(
            parse_datetime(b"20171130101010.564204-1001", default_offset).unwrap(),
            FixedOffset::west(10 * 3600 + 1 * 60)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 564_204)
        );
        assert_eq!(
            parse_datetime(b"20171130101010.564204-1001abcd", default_offset).unwrap(),
            FixedOffset::west(10 * 3600 + 1 * 60)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 564_204)
        );
        assert_eq!(
            parse_datetime(b"20171130101010.2-1100", default_offset).unwrap(),
            FixedOffset::west(11 * 3600)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 200_000)
        );
        assert_eq!(
            parse_datetime(b"20171130101010.0-1100", default_offset).unwrap(),
            FixedOffset::west(11 * 3600)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 0)
        );
        assert!(matches!(
            parse_datetime(b"20180101093059", default_offset),
            Err(Error::IncompleteValue {
                component: DateComponent::Fraction,
                ..
            })
        ));
        assert!(matches!(
            parse_datetime(b"201801010930", default_offset),
            Err(Error::IncompleteValue {
                component: DateComponent::Second,
                ..
            })
        ));
        assert!(matches!(
            parse_datetime(b"2018010109", default_offset),
            Err(Error::IncompleteValue {
                component: DateComponent::Minute,
                ..
            })
        ));
        assert!(matches!(
            parse_datetime(b"20180101", default_offset),
            Err(Error::UnexpectedEndOfElement { .. })
        ));
        assert!(matches!(
            parse_datetime(b"201801", default_offset),
            Err(Error::IncompleteValue {
                component: DateComponent::Day,
                ..
            })
        ));
        assert!(matches!(
            parse_datetime(b"1526", default_offset),
            Err(Error::IncompleteValue {
                component: DateComponent::Month,
                ..
            })
        ));

        
        let dt = parse_datetime(b"20171130101010.204+0100", default_offset).unwrap();
        assert_eq!(
            dt,
            FixedOffset::east(3600)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        assert_eq!(
            format!("{:?}", dt),
            "2017-11-30T10:10:10.204+01:00".to_string()
        );

        let dt = parse_datetime(b"20171130101010.204+0535", default_offset).unwrap();
        assert_eq!(
            dt,
            FixedOffset::east(5 * 3600 + 35 * 60)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        assert_eq!(
            format!("{:?}", dt),
            "2017-11-30T10:10:10.204+05:35".to_string()
        );
        assert_eq!(
            parse_datetime(b"20140505120101.204+0535", default_offset).unwrap(),
            FixedOffset::east(5 * 3600 + 35 * 60)
                .ymd(2014, 5, 5)
                .and_hms_micro(12, 1, 1, 204_000)
        );

        assert!(parse_datetime(b"", default_offset).is_err());
        assert!(parse_datetime(&[0x00_u8; 8], default_offset).is_err());
        assert!(parse_datetime(&[0xFF_u8; 8], default_offset).is_err());
        assert!(parse_datetime(&[b'0'; 8], default_offset).is_err());
        assert!(parse_datetime(&[b' '; 8], default_offset).is_err());
        assert!(parse_datetime(b"nope", default_offset).is_err());
        assert!(parse_datetime(b"2015dec", default_offset).is_err());
        assert!(parse_datetime(b"20151231162945.", default_offset).is_err());
        assert!(parse_datetime(b"20151130161445+", default_offset).is_err());
        assert!(parse_datetime(b"20151130161445+----", default_offset).is_err());
        assert!(parse_datetime(b"20151130161445. ", default_offset).is_err());
        assert!(parse_datetime(b"20151130161445. +0000", default_offset).is_err());
        assert!(parse_datetime(b"20100423164000.001+3", default_offset).is_err());
        assert!(parse_datetime(b"200809112945*1000", default_offset).is_err());
        assert!(parse_datetime(b"20171130101010.204+1", default_offset).is_err());
        assert!(parse_datetime(b"20171130101010.204+01", default_offset).is_err());
        assert!(parse_datetime(b"20171130101010.204+011", default_offset).is_err());
    }

    /*
    #[test]
    fn test_datetime_partial() {
        let default_offset = FixedOffset::east(0);
        assert_eq!(
            parse_datetime(b"201801010930", default_offset).unwrap(),
            FixedOffset::east(0).ymd(2018, 1, 1).and_hms(9, 30, 0)
        );
        assert_eq!(
            parse_datetime(b"19711231065003", default_offset).unwrap(),
            FixedOffset::east(0).ymd(1971, 12, 31).and_hms(6, 50, 3)
        );
        assert_eq!(
            parse_datetime(b"20171130101010.204", default_offset).unwrap(),
            FixedOffset::east(0)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        assert_eq!(
            parse_datetime(b"20180314000000.25", default_offset).unwrap(),
            FixedOffset::east(0)
                .ymd(2018, 03, 14)
                .and_hms_micro(0, 0, 0, 250_000)
        );
        let dt = parse_datetime(b"20171130101010.204+0100", default_offset).unwrap();
        assert_eq!(
            dt,
            FixedOffset::east(3600)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        assert_eq!(
            format!("{:?}", dt),
            "2017-11-30T10:10:10.204+01:00".to_string()
        );

        assert_eq!(
            parse_datetime(b"20171130101010.204-1000", default_offset).unwrap(),
            FixedOffset::west(10 * 3600)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        let dt = parse_datetime(b"20171130101010.204+0535", default_offset).unwrap();
        assert_eq!(
            dt,
            FixedOffset::east(5 * 3600 + 35 * 60)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        assert_eq!(
            format!("{:?}", dt),
            "2017-11-30T10:10:10.204+05:35".to_string()
        );
        assert_eq!(
            parse_datetime(b"20140426", default_offset).unwrap(),
            FixedOffset::east(0).ymd(2014, 4, 26).and_hms(0, 0, 0)
        );
        assert_eq!(
            parse_datetime(b"2014+0535", default_offset).unwrap(),
            FixedOffset::east(5 * 3600 + 35 * 60)
                .ymd(2014, 1, 1)
                .and_hms(0, 0, 0)
        );
        assert_eq!(
            parse_datetime(b"20140505+0535", default_offset).unwrap(),
            FixedOffset::east(5 * 3600 + 35 * 60)
                .ymd(2014, 5, 5)
                .and_hms(0, 0, 0)
        );
        assert_eq!(
            parse_datetime(b"20140505120101.204+0535", default_offset).unwrap(),
            FixedOffset::east(5 * 3600 + 35 * 60)
                .ymd(2014, 5, 5)
                .and_hms_micro(12, 1, 1, 204_000)
        );

        assert!(parse_datetime(b"", default_offset).is_err());
        assert!(parse_datetime(&[0x00_u8; 8], default_offset).is_err());
        assert!(parse_datetime(&[0xFF_u8; 8], default_offset).is_err());
        assert!(parse_datetime(&[b'0'; 8], default_offset).is_err());
        assert!(parse_datetime(&[b' '; 8], default_offset).is_err());
        assert!(parse_datetime(b"nope", default_offset).is_err());
        assert!(parse_datetime(b"2015dec", default_offset).is_err());
        assert!(parse_datetime(b"20151231162945.", default_offset).is_err());
        assert!(parse_datetime(b"20151130161445+", default_offset).is_err());
        assert!(parse_datetime(b"20151130161445+----", default_offset).is_err());
        assert!(parse_datetime(b"20151130161445. ", default_offset).is_err());
        assert!(parse_datetime(b"20151130161445. +0000", default_offset).is_err());
        assert!(parse_datetime(b"20100423164000.001+3", default_offset).is_err());
        assert!(parse_datetime(b"200809112945*1000", default_offset).is_err());
        assert!(parse_datetime(b"20171130101010.204+1", default_offset).is_err());
        assert!(parse_datetime(b"20171130101010.204+01", default_offset).is_err());
        assert!(parse_datetime(b"20171130101010.204+011", default_offset).is_err());
    }*/
}
