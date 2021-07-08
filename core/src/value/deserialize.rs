//! Parsing of primitive values
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use snafu::{Backtrace, OptionExt, Snafu};
use std::ops::{Add, Mul, Sub};

const Z: i32 = b'0' as i32;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Unexpected end of element"))]
    UnexpectedEndOfElement { backtrace: Backtrace },
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
    #[snafu(display("Invalid number length: it is {}, but must be between 1 and 9", len))]
    InvalidNumberLength { len: usize, backtrace: Backtrace },
    #[snafu(display("Invalid number token: got '{}', but must be a digit in '0'..='9'", *value as char))]
    InvalidNumberToken { value: u8, backtrace: Backtrace },
    #[snafu(display("Invalid time zone sign token: got '{}', but must be '+' or '-'", *value as char))]
    InvalidTimeZoneSignToken { value: u8, backtrace: Backtrace },
}

type Result<T, E = Error> = std::result::Result<T, E>;

/** Decode a single DICOM Date (DA) into a `NaiveDate` value.
 */
pub fn parse_date(buf: &[u8]) -> Result<(NaiveDate, &[u8])> {
    // YYYY(MM(DD)?)?
    match buf.len() {
        0 | 5 | 7 => UnexpectedEndOfElement.fail(),
        1..=4 => {
            let year = read_number(buf)?;
            let date: Result<_> = NaiveDate::from_ymd_opt(year, 1, 1).context(InvalidDateTimeZone);
            Ok((date?, &[]))
        }
        6 => {
            let year = read_number(&buf[0..4])?;
            let month = (i32::from(buf[4]) - Z) * 10 + i32::from(buf[5]) - Z;
            let date: Result<_> =
                NaiveDate::from_ymd_opt(year, month as u32, 1).context(InvalidDateTimeZone);
            Ok((date?, &buf[6..]))
        }
        len => {
            debug_assert!(len >= 8);
            let year = read_number(&buf[0..4])?;
            let month = (i32::from(buf[4]) - Z) * 10 + i32::from(buf[5]) - Z;
            let day = (i32::from(buf[6]) - Z) * 10 + i32::from(buf[7]) - Z;
            let date: Result<_> = NaiveDate::from_ymd_opt(year, month as u32, day as u32)
                .context(InvalidDateTimeZone);
            Ok((date?, &buf[8..]))
        }
    }
}

/** Decode a single DICOM Time (TM) into a `NaiveTime` value.
 */
pub fn parse_time(buf: &[u8]) -> Result<(NaiveTime, &[u8])> {
    parse_time_impl(buf, false)
}

/// A version of `NativeTime::from_hms` which returns a more informative error.
fn naive_time_from_components(
    hour: u32,
    minute: u32,
    second: u32,
    micro: u32,
) -> Result<NaiveTime> {
    if hour >= 24 {
        InvalidDateTimeHour { value: hour }.fail()
    } else if minute >= 60 {
        InvalidDateTimeMinute { value: minute }.fail()
    } else if second >= 60 {
        InvalidDateTimeSecond { value: second }.fail()
    } else if micro >= 2_000_000 {
        InvalidDateTimeMicrosecond { value: micro }.fail()
    } else {
        Ok(NaiveTime::from_hms_micro(hour, minute, second, micro))
    }
}

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
            match buf[6] {
                b'.' => { /* do nothing */ }
                b'+' | b'-' if for_datetime => { /* do nothing */ }
                c => return UnexpectedAfterDateToken { value: c }.fail(),
            }
            let buf = &buf[7..];
            // read at most 6 bytes
            let mut n = usize::min(6, buf.len());
            if for_datetime {
                // check for time zone suffix, restrict fraction size accordingly
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
            let time =
                naive_time_from_components(hour as u32, minute as u32, second as u32, fract)?;
            Ok((time, &buf[n..]))
        }
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
pub fn parse_datetime(buf: &[u8], dt_utc_offset: FixedOffset) -> Result<DateTime<FixedOffset>> {
    let (date, rest) = parse_date(buf)?;
    if buf.len() <= 8 {
        return Ok(FixedOffset::east(0).from_utc_date(&date).and_hms(0, 0, 0));
    }
    let buf = rest;
    let (time, buf) = parse_time_impl(buf, true)?;
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
        1 | 2 => return UnexpectedEndOfElement.fail(),
        _ => {
            let tz_sign = buf[0];
            let buf = &buf[1..];
            let (tz_h, tz_m) = match buf.len() {
                4 => {
                    let (h_buf, m_buf) = buf.split_at(2);
                    let tz_h : i32 = read_number(h_buf)?;
                    let tz_m : i32 = read_number(m_buf)?;
                    (tz_h, tz_m)
                },
                _ => return UnexpectedEndOfElement.fail(),
            };
            let s = (tz_h * 60 + tz_m) * 60;
            match tz_sign {
                b'+' => FixedOffset::east(s),
                b'-' => FixedOffset::west(s),
                c => return InvalidTimeZoneSignToken { value: c }.fail(),
            }
        }
    };

    offset
        .from_utc_date(&date)
        .and_time(time)
        .context(InvalidDateTimeZone)
}

#[cfg(test)]
mod tests {
    use super::{parse_date, parse_datetime, parse_time, parse_time_impl};
    use chrono::{FixedOffset, NaiveDate, NaiveTime, TimeZone};

    #[test]
    fn test_parse_date() {
        assert_eq!(
            parse_date(b"20180101").unwrap(),
            (NaiveDate::from_ymd(2018, 1, 1), &[][..])
        );
        assert_eq!(
            parse_date(b"19711231").unwrap(),
            (NaiveDate::from_ymd(1971, 12, 31), &[][..])
        );
        assert_eq!(
            parse_date(b"197112").unwrap(),
            (NaiveDate::from_ymd(1971, 12, 1), &[][..])
        );
        assert_eq!(
            parse_date(b"20140426").unwrap(),
            (NaiveDate::from_ymd(2014, 4, 26), &[][..])
        );
        assert_eq!(
            parse_date(b"20180101xxxx").unwrap(),
            (NaiveDate::from_ymd(2018, 1, 1), &b"xxxx"[..])
        );
        assert_eq!(
            parse_date(b"19000101").unwrap(),
            (NaiveDate::from_ymd(1900, 1, 1), &[][..])
        );
        assert_eq!(
            parse_date(b"19620728").unwrap(),
            (NaiveDate::from_ymd(1962, 7, 28), &[][..])
        );
        assert_eq!(
            parse_date(b"1902").unwrap(),
            (NaiveDate::from_ymd(1902, 1, 1), &[][..])
        );
        assert!(parse_date(b"").is_err());
        assert!(parse_date(b"        ").is_err());
        assert!(parse_date(b"--------").is_err());
        assert!(parse_date(&[0x00_u8; 8]).is_err());
        assert!(parse_date(&[0xFF_u8; 8]).is_err());
        assert!(parse_date(&[b'0'; 8]).is_err());
        assert!(parse_date(b"19991313").is_err());
        assert!(parse_date(b"20180229").is_err());
        assert!(parse_date(b"nothing!").is_err());
        assert!(parse_date(b"2012dec").is_err());
    }

    #[test]
    fn test_time() {
        assert_eq!(
            parse_time(b"10").unwrap(),
            (NaiveTime::from_hms(10, 0, 0), &[][..])
        );
        assert_eq!(
            parse_time(b"0755").unwrap(),
            (NaiveTime::from_hms(7, 55, 0), &[][..])
        );
        assert_eq!(
            parse_time(b"075500").unwrap(),
            (NaiveTime::from_hms(7, 55, 0), &[][..])
        );
        assert_eq!(
            parse_time(b"065003").unwrap(),
            (NaiveTime::from_hms(6, 50, 3), &[][..])
        );
        assert_eq!(
            parse_time(b"075501.5").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 500_000), &[][..])
        );
        assert_eq!(
            parse_time(b"075501.58").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 580_000), &[][..])
        );
        assert_eq!(
            parse_time(b"075501.58").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 580_000), &[][..])
        );
        assert_eq!(
            parse_time(b"101010.204").unwrap(),
            (NaiveTime::from_hms_micro(10, 10, 10, 204_000), &[][..])
        );
        assert_eq!(
            parse_time(b"075501.123456").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 123_456), &[][..])
        );
        assert_eq!(
            parse_time_impl(b"075501.123456-05:00", true).unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 123_456), &b"-05:00"[..])
        );
        assert_eq!(
            parse_time(b"235959.99999").unwrap(),
            (NaiveTime::from_hms_micro(23, 59, 59, 999_990), &[][..])
        );
        assert_eq!(
            parse_time(b"235959.123456max precision").unwrap(),
            (
                NaiveTime::from_hms_micro(23, 59, 59, 123_456),
                &b"max precision"[..]
            )
        );
        assert_eq!(
            parse_time_impl(b"235959.999999+01:00", true).unwrap(),
            (
                NaiveTime::from_hms_micro(23, 59, 59, 999_999),
                &b"+01:00"[..]
            )
        );
        assert_eq!(
            parse_time(b"235959.792543").unwrap(),
            (NaiveTime::from_hms_micro(23, 59, 59, 792_543), &[][..])
        );
        assert_eq!(
            parse_time(b"100003.123456...").unwrap(),
            (NaiveTime::from_hms_micro(10, 0, 3, 123_456), &b"..."[..])
        );
        assert_eq!(
            parse_time(b"000000.000000").unwrap(),
            (NaiveTime::from_hms(0, 0, 0), &[][..])
        );
        assert!(parse_time(b"075501.123......").is_err());
        assert!(parse_date(b"").is_err());
        assert!(parse_date(&[0x00_u8; 6]).is_err());
        assert!(parse_date(&[0xFF_u8; 6]).is_err());
        assert!(parse_date(b"      ").is_err());
        assert!(parse_date(b"------").is_err());
        assert!(parse_date(b"------.----").is_err());
        assert!(parse_date(b"235959.9999").is_err());
        assert!(parse_date(b"075501.").is_err());
        assert!(parse_date(b"075501.----").is_err());
        assert!(parse_date(b"nope").is_err());
        assert!(parse_date(b"235800.0a").is_err());
    }

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
    }
}
