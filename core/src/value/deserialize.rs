//! Parsing of primitive values
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use snafu::{Backtrace, OptionExt, Snafu};
use std::ops::{Add, Mul, Sub};

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
    #[snafu(display("No Range Separator Present"))]
    NoRangeSeparator { backtrace: Backtrace },
    #[snafu(display("End {} before Start {}", end, start))]
    RangeInversion {
        start: String,
        end: String,
        backtrace: Backtrace,
    },
    #[snafu(display("Start {} == End {}", start, end))]
    RangeIsZero {
        start: String,
        end: String,
        backtrace: Backtrace,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

/** Decode a single DICOM Date (DA) into a `NaiveDate` value.
 */
pub fn parse_date(buf: &[u8]) -> Result<(NaiveDate, &[u8])> {
    // YYYY(MM(DD)?)?
    match buf.len() {
        0 | 5 | 7 => UnexpectedEndOfElement.fail(),
        4 => {
            let year = read_number(buf)?;
            let date: Result<_> = NaiveDate::from_ymd_opt(year, 1, 1).context(InvalidDateTimeZone);
            Ok((date?, &[]))
        }
        6 => {
            let year = read_number(&buf[0..4])?;
            let month: u32 = read_number(&buf[4..6])?;
            let date: Result<_> =
                NaiveDate::from_ymd_opt(year, month, 1).context(InvalidDateTimeZone);
            Ok((date?, &buf[6..]))
        }
        len => {
            debug_assert!(len >= 8);
            let year = read_number(&buf[0..4])?;
            let (month, day, rest) = match buf[4] {
                /* MM and DD not present, UTC follows*/
                b'-' | b'+' => (1, 1, &buf[4..]),
                _ => {
                    /* Attempt to parse MM */
                    let m: u32 = read_number(&buf[4..6])?;
                    let (d, r) = match buf[6] {
                        /* DD not present, UTC follows */
                        b'-' | b'+' => (1, &buf[6..]),
                        _ => (read_number(&buf[6..8])?, &buf[8..]), /* Attempt to parse DD */
                    };
                    (m, d, r)
                }
            };

            let date: Result<_> =
                NaiveDate::from_ymd_opt(year, month, day).context(InvalidDateTimeZone);
            Ok((date?, rest))
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
}

macro_rules! check_range {
    ($s: expr, $e: expr) => {
        if $s == $e {
            RangeIsZero {
                start: $s.to_string(),
                end: $e.to_string(),
            }
            .fail()
        } else if $s < $e {
            Ok((Some($s), Some($e)))
        } else {
            RangeInversion {
                start: $s.to_string(),
                end: $e.to_string(),
            }
            .fail()
        }
    };
}

/**
 *  Looks for a range separator '-'.
 *  Returns a tuple of two Option\<NaiveDate\>
 *  None means no upper or lower range is present
 */
pub fn parse_date_range(buf: &[u8]) -> Result<(Option<NaiveDate>, Option<NaiveDate>)> {
    // minimum length of one valid Date (YYYY) and one '-' separator
    if buf.len() < 5 {
        return UnexpectedEndOfElement.fail();
    }

    if let Some(separator) = buf.iter().position(|e| *e == b'-') {
        let (start, end) = buf.split_at(separator);
        let end = &end[1..];
        match separator {
            0 => Ok((None, Some(parse_date(end)?.0))),
            i if i == buf.len() - 1 => Ok((Some(parse_date(start)?.0), None)),
            _ => {
                let (s, e) = (parse_date(start)?.0, parse_date(end)?.0);
                check_range!(s, e)
            }
        }
    } else {
        NoRangeSeparator.fail()
    }
}

/**
 *  Looks for a range separator '-'.
 *  Returns a tuple of two Option\<NaiveTime\>
 *  None means no upper or lower range is present
 */
pub fn parse_time_range(buf: &[u8]) -> Result<(Option<NaiveTime>, Option<NaiveTime>)> {
    // minimum length of one valid Time (HH) and one '-' separator
    if buf.len() < 3 {
        return UnexpectedEndOfElement.fail();
    }

    if let Some(separator) = buf.iter().position(|e| *e == b'-') {
        let (start, end) = buf.split_at(separator);
        let end = &end[1..];
        match separator {
            0 => Ok((None, Some(parse_time(end)?.0))),
            i if i == buf.len() - 1 => Ok((Some(parse_time(start)?.0), None)),
            _ => {
                let (s, e) = (parse_time(start)?.0, parse_time(end)?.0);
                check_range!(s, e)
            }
        }
    } else {
        NoRangeSeparator.fail()
    }
}

/**
 *  Looks for a range separator '-'.
 *  Returns a tuple of two Option\<DateTime\>
 *  None means no upper or lower range is present
 */
pub fn parse_datetime_range(
    buf: &[u8],
    dt_utc_offset: FixedOffset,
) -> Result<(Option<DateTime<FixedOffset>>, Option<DateTime<FixedOffset>>)> {
    // minimum length of one valid DateTime (YYYY) and one '-' separator
    if buf.len() < 5 {
        return UnexpectedEndOfElement.fail();
    }

    if let Some(separator) = buf
        .iter()
        .enumerate()
        .find(|(i, c)| {
            match **c == b'-' {
                true => {
                    match i {
                        /* empty separator in the beginning */
                        0 => true,
                        /* empty separator at the end */
                        x if *x == buf.len() - 1 => true,
                        x if *x < buf.len() - 6 => {
                            match buf[x + 5] {
                                // separator present in 5 bytes, so assume this is an offset sign
                                b'-' => false,
                                _ => true,
                            }
                        }
                        /* for a very short YYYY-YYYY range case */
                        4 if buf.len() == 9 => true,
                        _ => false,
                    }
                }
                false => false,
            }
        })
        .map(|(i, _c)| i)
    {
        let (start, end) = buf.split_at(separator);
        let end = &end[1..];
        match separator {
            0 => Ok((None, Some(parse_datetime(end, dt_utc_offset)?))),
            i if i == buf.len() - 1 => Ok((Some(parse_datetime(start, dt_utc_offset)?), None)),
            _ => {
                let (s, e) = (
                    parse_datetime(start, dt_utc_offset)?,
                    parse_datetime(end, dt_utc_offset)?,
                );
                check_range!(s, e)
            }
        }
    } else {
        NoRangeSeparator.fail()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_date, parse_date_range, parse_datetime, parse_datetime_range, parse_time,
        parse_time_impl, parse_time_range,
    };
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
        assert_eq!(
            parse_date(b"1902+0101").unwrap(),
            (NaiveDate::from_ymd(1902, 1, 1), &b"+0101"[..][..])
        );
        assert_eq!(
            parse_date(b"1902-0101").unwrap(),
            (NaiveDate::from_ymd(1902, 1, 1), &b"-0101"[..][..])
        );

        assert_eq!(
            parse_date(b"190204-0101").unwrap(),
            (NaiveDate::from_ymd(1902, 4, 1), &b"-0101"[..][..])
        );

        assert_eq!(
            parse_date(b"19020404-0101").unwrap(),
            (NaiveDate::from_ymd(1902, 4, 4), &b"-0101"[..][..])
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
    }
    #[test]
    fn test_parse_date_range() {
        assert!(parse_date_range("1914-".as_bytes()).is_ok());
        assert!(parse_date_range("-2010".as_bytes()).is_ok());

        assert_eq!(
            parse_date_range("-201003".as_bytes()).unwrap(),
            (None, Some(NaiveDate::from_ymd(2010, 3, 1)))
        );
        assert_eq!(
            parse_date_range("20100305-".as_bytes()).unwrap(),
            (Some(NaiveDate::from_ymd(2010, 3, 5)), None)
        );

        assert!(parse_date_range("718-".as_bytes()).is_err());
        assert!(parse_date_range("1914-1900".as_bytes()).is_err());
        assert!(parse_date_range("19140101-19140101".as_bytes()).is_err());
    }

    #[test]
    fn test_parse_time_range() {
        assert!(parse_time_range("01-".as_bytes()).is_ok());
        assert!(parse_time_range("010505.1234-".as_bytes()).is_ok());

        assert_eq!(
            parse_time_range("010505.1234-".as_bytes()).unwrap(),
            (Some(NaiveTime::from_hms_micro(1, 5, 5, 123400)), None)
        );
        assert_eq!(
            parse_time_range("-010505.12".as_bytes()).unwrap(),
            (None, Some(NaiveTime::from_hms_micro(1, 5, 5, 120_000)))
        );

        assert!(parse_time_range("1-".as_bytes()).is_err());
        assert!(parse_time_range("010505.123+0101-".as_bytes()).is_err());
        assert!(parse_time_range("1530-1100".as_bytes()).is_err());
        assert!(parse_time_range("153001-153001".as_bytes()).is_err());
    }

    #[test]
    fn test_parse_datetime_range() {
        let o = FixedOffset::east(0);
        assert!(parse_datetime_range(
            "19700101152430.123456-0101-19800101152430.123456-0101".as_bytes(),
            o
        )
        .is_ok());
        assert!(parse_datetime_range(
            "19700101152430.123456-19800101152430.123456-0101".as_bytes(),
            o
        )
        .is_ok());
        assert!(parse_datetime_range("-19800101152430.1234-1040".as_bytes(), o).is_ok());
        assert!(parse_datetime_range(
            "19700101152430.1234-1101-19800101152430.123456".as_bytes(),
            o
        )
        .is_ok());
        assert!(parse_datetime_range("19700101152430.1234-1101-".as_bytes(), o).is_ok());
        assert!(parse_datetime_range(
            "19700101152430.123456+0101-19800101152430.123456-0101".as_bytes(),
            o
        )
        .is_ok());
        assert!(parse_datetime_range(
            "19700101152430.1234-19800101152430.123456-1040".as_bytes(),
            o
        )
        .is_ok());
        assert!(parse_datetime_range(
            "19700101152430.123456+1101-19800101152430.123456".as_bytes(),
            o
        )
        .is_ok());
        assert!(parse_datetime_range("19700101152430.123456+1101-".as_bytes(), o).is_ok());
        assert!(parse_datetime_range(
            "19700101152430.123456-0101-19800101152430.123456+0101".as_bytes(),
            o
        )
        .is_ok());
        assert!(parse_datetime_range(
            "19700101152430.123456-19800101152430.123456+1040".as_bytes(),
            o
        )
        .is_ok());
        assert!(parse_datetime_range("-19800101152430.123+0101".as_bytes(), o).is_ok());
        assert!(parse_datetime_range("1980-".as_bytes(), o).is_ok());
        assert!(parse_datetime_range("1980+0100-".as_bytes(), o).is_ok());

        assert_eq!(
            parse_datetime_range(
                "19700101152430.123456-0100-19800101152430.123456-0101".as_bytes(),
                o
            )
            .unwrap(),
            (
                Some(
                    FixedOffset::west(3600)
                        .ymd(1970, 1, 1)
                        .and_hms_micro(15, 24, 30, 123456)
                ),
                Some(
                    FixedOffset::west(3660)
                        .ymd(1980, 1, 1)
                        .and_hms_micro(15, 24, 30, 123456)
                )
            )
        );

        assert_eq!(
            parse_datetime_range("-19800101152430.123456-0101".as_bytes(), o).unwrap(),
            (
                None,
                Some(
                    FixedOffset::west(3660)
                        .ymd(1980, 1, 1)
                        .and_hms_micro(15, 24, 30, 123456)
                )
            )
        );

        assert_eq!(
            parse_datetime_range("19700101152430.123456-0100-".as_bytes(), o).unwrap(),
            (
                Some(
                    FixedOffset::west(3600)
                        .ymd(1970, 1, 1)
                        .and_hms_micro(15, 24, 30, 123456)
                ),
                None
            )
        );

        assert!(parse_datetime_range("1970-1970".as_bytes(), o).is_err());
        assert!(parse_datetime_range("1980-1970".as_bytes(), o).is_err());
        assert!(parse_datetime_range("bogus-19800101152430.123+0101".as_bytes(), o).is_err());
        assert!(parse_datetime_range("19700101152430.1234-1101-bogus".as_bytes(), o).is_err());
        assert!(parse_datetime_range("123-".as_bytes(), o).is_err());
    }
}
