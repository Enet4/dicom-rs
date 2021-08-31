//! Parsing of primitive values
use crate::value::partial::{
    check_component, AsTemporalRange, DateComponent, Error as PartialValuesError, PartialDate,
    PartialTime,
};
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::ops::{Add, Mul, Sub};
use std::convert::TryFrom;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Unexpected end of element"))]
    UnexpectedEndOfElement { backtrace: Backtrace },
    /*#[snafu(display(
        "{:?} has invalid value:  {}, must be in {:?}",
        component,
        value,
        range
    ))]
    InvalidComponent {
        component: DateComponent,
        value: u32,
        range: RangeInclusive<u32>,
        backtrace: Backtrace,
    },*/
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

/*
/**
 * Throws a detailed `InvalidComponent` error if Date / Time components are out of range.
 */
pub fn check_component<T>(component: DateComponent, value: T) -> Result<()>
    where T: Into<u32>  {
    let range = match component {
        DateComponent::Year => 0..=9_999,
        DateComponent::Month => 1..=12,
        DateComponent::Day => 1..=31,
        DateComponent::Hour => 0..=23,
        DateComponent::Minute => 0..=59,
        DateComponent::Second => 0..=59,
        DateComponent::Fraction => 0..=1_999_999,
        DateComponent::UTCOffset => 0..=86_399,
    };

    let value : u32 = value.into();
    if range.contains(&value) {
        Ok(())
    } else {
        InvalidComponent {
            component,
            value,
            range,
        }
        .fail()
    }
}
*/

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
                    let mul :u8 = 6 - u8::try_from(n).unwrap();
                    Ok((
                        PartialTime::from_hmsf(&hour, &minute, &second, &fraction, &mul)
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
pub fn parse_time(buf: &[u8]) -> Result<NaiveTime> {
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
                let n = usize::min(6, buf.len());
                let mut fraction: u32 = read_number(&buf[0..n])?;
                let mut acc = n;
                while acc < 6 {
                    fraction *= 10;
                    acc += 1;
                }
                check_component(DateComponent::Fraction, &fraction).context(InvalidComponent)?;
                Ok(NaiveTime::from_hms_micro(hour, minute, second, fraction))
            }
        }
        _ => UnexpectedEndOfElement.fail(),
    }
}

/*
fn parse_time_impl(buf: &[u8], for_datetime: bool) -> Result<(NaiveTime, &[u8])> {
    const Z: i32 = b'0' as i32;
    // HH(MM(SS(.F{1,6})?)?)?

    match buf.len() {
        0 | 1 | 3 | 5 | 7 => UnexpectedEndOfElement.fail(),
        2 => {
            let hour = (i32::from(buf[0]) - Z) * 10 + i32::from(buf[1]) - Z;
            let time = naive_time_from_components(hour as u32, 0, 0, 0)?;
            Ok((time, &buf[2..]))
        }
        4 => {
            let hour = (i32::from(buf[0]) - Z) * 10 + i32::from(buf[1]) - Z;
            let minute = (i32::from(buf[2]) - Z) * 10 + i32::from(buf[3]) - Z;
            let time = naive_time_from_components(hour as u32, minute as u32, 0, 0)?;
            Ok((time, &buf[4..]))
        }
        6 => {
            let hour = (i32::from(buf[0]) - Z) * 10 + i32::from(buf[1]) - Z;
            let minute = (i32::from(buf[2]) - Z) * 10 + i32::from(buf[3]) - Z;
            let second = (i32::from(buf[4]) - Z) * 10 + i32::from(buf[5]) - Z;

            let time = naive_time_from_components(hour as u32, minute as u32, second as u32, 0)?;
            Ok((time, &buf[6..]))
        }
        _ => {
            let hour = (i32::from(buf[0]) - Z) * 10 + i32::from(buf[1]) - Z;
            let minute = (i32::from(buf[2]) - Z) * 10 + i32::from(buf[3]) - Z;
            let second = (i32::from(buf[4]) - Z) * 10 + i32::from(buf[5]) - Z;
            let (fract, rest) = match buf[6] {
                /* fraction present */
                b'.' => {
                    let buf = &buf[7..];
                    // read at most 6 bytes
                    let mut n = usize::min(6, buf.len());
                    if for_datetime {
                        // check for UTC after fraction, restrict fraction size accordingly
                        if let Some(i) = buf.iter().position(|v| *v == b'+' || *v == b'-') {
                            n = i;
                        }
                    }
                    let mut fract: u32 = read_number(&buf[0..n])?;
                    let mut acc = n;
                    while acc < 6 {
                        fract *= 10;
                        acc += 1;
                    }
                    (fract, &buf[n..])
                }
                /* no fraction, but UTC offset present, ok only for DT */
                b'+' | b'-' if for_datetime => (0, &buf[6..]),
                c => return UnexpectedAfterDateToken { value: c }.fail(),
            };

            let time =
                naive_time_from_components(hour as u32, minute as u32, second as u32, fract)?;
            Ok((time, rest))
        }
    }
}
*/

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

/// Retrieve a DICOM date-time from the given text, while assuming the given
/// UTC offset.
/*
pub fn parse_datetime(buf: &[u8], dt_utc_offset: FixedOffset) -> Result<DateTime<FixedOffset>> {
    let (date, rest) = parse_date_partial(buf)?;
    let date = date.earliest().context(PartialValue)?;

    // too short for full DT(date,time) without offset, handle date without time component
    if buf.len() <= 8 {
        return Ok(FixedOffset::east(0).from_utc_date(&date).and_hms(0, 0, 0));
    }
    let buf = rest;
    // after YYYY all DT components are optional, fail time parsing gracefully
    // with default values, assume UTC offset can still be present
    let (time, buf) =
        parse_time_impl(buf, true).unwrap_or((naive_time_from_components(0, 0, 0, 0)?, rest));

    let len = buf.len();
    let offset = match len {
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
        5 => {
            let tz_sign = buf[0];
            let buf = &buf[1..];
            let (h_buf, m_buf) = buf.split_at(2);
            let tz_h: i32 = read_number(h_buf)?;
            let tz_m: i32 = read_number(m_buf)?;
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
            NaiveTime::from_hms_micro(10, 0, 0, 100_000)
        );
        assert_eq!(
            parse_time(b"235959.0123").unwrap(),
            NaiveTime::from_hms_micro(23, 59, 59, 12_300)
        );
        // only parses 6 digit precision as in DICOM standard
        assert_eq!(
            parse_time(b"235959.1234567").unwrap(),
            NaiveTime::from_hms_micro(23, 59, 59, 123_456)
        );
        assert_eq!(
            parse_time(b"000000.000000").unwrap(),
            NaiveTime::from_hms(0, 0, 0)
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
            (PartialTime::Fraction(7, 55, 1, 5, 5), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075501.123").unwrap(),
            (PartialTime::Fraction(7, 55, 1, 123, 3), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075501.999999").unwrap(),
            (PartialTime::Fraction(7, 55, 1, 999_999, 0), &[][..])
        );
        assert_eq!(
            parse_time_partial(b"075501.9999994").unwrap(),
            (PartialTime::Fraction(7, 55, 1, 999_999, 0), &b"4"[..])
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
    /*
    #[test]
    fn test_datetime() {
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
