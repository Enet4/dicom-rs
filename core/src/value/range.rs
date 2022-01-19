//! Handling of date, time, date-time ranges. Needed for range matching.
//! Parsing into ranges happens via partial precision  structures (DicomDate, DicomTime,
//! DicomDatime) so ranges can handle null components in date, time, date-time values.
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};

use crate::value::deserialize::{
    parse_date_partial, parse_datetime_partial, parse_time_partial, Error as DeserializeError,
};
use crate::value::partial::{DateComponent, DicomDate, DicomDateTime, DicomTime, Precision};

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Unexpected end of element"))]
    UnexpectedEndOfElement { backtrace: Backtrace },
    #[snafu(display("Failed to parse value"))]
    Parse {
        #[snafu(backtrace)]
        source: DeserializeError,
    },
    #[snafu(display("End {} is before start {}", end, start))]
    RangeInversion {
        start: String,
        end: String,
        backtrace: Backtrace,
    },
    #[snafu(display("No range separator present"))]
    NoRangeSeparator { backtrace: Backtrace },
    #[snafu(display("Date-time range can contain 1-3 '-' characters, {} were found", value))]
    SeparatorCount { value: usize, backtrace: Backtrace },
    #[snafu(display("Invalid date-time"))]
    InvalidDateTime { backtrace: Backtrace },
    #[snafu(display(
        "Cannot convert from an imprecise value. This value represents a date / time range"
    ))]
    ImpreciseValue { backtrace: Backtrace },
    #[snafu(display("Date is invalid"))]
    InvalidDate { backtrace: Backtrace },
    #[snafu(display("Time is invalid"))]
    InvalidTime { backtrace: Backtrace },
}
type Result<T, E = Error> = std::result::Result<T, E>;

/// The DICOM protocol accepts date / time values with null components.
/// Imprecise values are to be handled as date / time ranges.
/// This trait is implemented by date / time structures with partial precision.
/// If the date / time structure is not precise, it is up to the user to call one of these
/// methods to retrieve a suitable  `chrono` value.
///
/// # Examples
///
/// ```
/// # use dicom_core::value::{C, PrimitiveValue};
/// # use smallvec::smallvec;
/// # use std::error::Error;
/// use chrono::{NaiveDate, NaiveTime};
/// use dicom_core::value::{AsRange, DicomDate, DicomTime, TimeRange};
/// # fn main() -> Result<(), Box<dyn Error>> {
///
/// let dicom_date = DicomDate::from_ym(2010,1)?;
/// assert_eq!(dicom_date.is_precise(), false);
/// assert_eq!(
///     dicom_date.earliest()?,
///     NaiveDate::from_ymd(2010,1,1)
/// );
/// assert_eq!(
///     dicom_date.latest()?,
///     NaiveDate::from_ymd(2010,1,31)
/// );
///
/// let dicom_time = DicomTime::from_hm(10,0)?;
/// assert_eq!(
///     dicom_time.range()?,
///     TimeRange::from_start_to_end(NaiveTime::from_hms(10, 0, 0),
///         NaiveTime::from_hms_micro(10, 0, 59, 999_999))?
/// );
/// // only a time with 6 digits second fraction is considered precise
/// assert!(dicom_time.exact().is_err());
///
/// # Ok(())
/// # }
/// ```
pub trait AsRange: Precision {
    type Item: PartialEq + PartialOrd;
    type Range;
    /// Returns a corresponding `chrono` value, if the partial precision structure has full accuracy.
    fn exact(&self) -> Result<Self::Item> {
        if self.is_precise() {
            Ok(self.earliest()?)
        } else {
            ImpreciseValueSnafu.fail()
        }
    }

    /// Returns the earliest possible `chrono` value from a partial precision structure.
    /// Missing components default to 1 (days, months) or 0 (hours, minutes, ...)
    /// If structure contains invalid combination of `DateComponent`s, it fails.
    fn earliest(&self) -> Result<Self::Item>;

    /// Returns the latest possible `chrono` value from a partial precision structure.
    /// If structure contains invalid combination of `DateComponent`s, it fails.
    fn latest(&self) -> Result<Self::Item>;

    /// Returns a tuple of the earliest and latest possible value from a partial precision structure.
    fn range(&self) -> Result<Self::Range>;

    /// Returns `true` if partial precision structure has the maximum possible accuracy.
    /// For fraction of a second, the full 6 digits are required for the value to be precise.
    fn is_precise(&self) -> bool {
        let e = self.earliest();
        let l = self.latest();

        e.is_ok() && l.is_ok() && e.ok() == l.ok()
    }
}

impl AsRange for DicomDate {
    type Item = NaiveDate;
    type Range = DateRange;
    fn earliest(&self) -> Result<NaiveDate> {
        let (y, m, d) = {
            (
                *self.year() as i32,
                *self.month().unwrap_or(&1) as u32,
                *self.day().unwrap_or(&1) as u32,
            )
        };
        NaiveDate::from_ymd_opt(y, m, d).context(InvalidDateSnafu)
    }

    fn latest(&self) -> Result<NaiveDate> {
        let (y, m, d) = (
            self.year(),
            self.month().unwrap_or(&12),
            match self.day() {
                Some(d) => *d as u32,
                None => {
                    let y = self.year();
                    let m = self.month().unwrap_or(&12);
                    if m == &12 {
                        NaiveDate::from_ymd(*y as i32 + 1, 1, 1)
                    } else {
                        NaiveDate::from_ymd(*y as i32, *m as u32 + 1, 1)
                    }
                    .signed_duration_since(NaiveDate::from_ymd(*y as i32, *m as u32, 1))
                    .num_days() as u32
                }
            },
        );

        NaiveDate::from_ymd_opt(*y as i32, *m as u32, d).context(InvalidDateSnafu)
    }

    fn range(&self) -> Result<DateRange> {
        let start = self.earliest()?;
        let end = self.latest()?;
        DateRange::from_start_to_end(start, end)
    }
}

impl AsRange for DicomTime {
    type Item = NaiveTime;
    type Range = TimeRange;
    fn earliest(&self) -> Result<NaiveTime> {
        let (h, m, s, f) = (
            self.hour(),
            self.minute().unwrap_or(&0),
            self.second().unwrap_or(&0),
            match self.fraction_and_precision() {
                None => 0,
                Some((f, fp)) => *f * u32::pow(10, 6 - <u32>::from(*fp)),
            },
        );

        NaiveTime::from_hms_micro_opt((*h).into(), (*m).into(), (*s).into(), f).context(InvalidTimeSnafu)
    }
    fn latest(&self) -> Result<NaiveTime> {
        let (h, m, s, f) = (
            self.hour(),
            self.minute().unwrap_or(&59),
            self.second().unwrap_or(&59),
            match self.fraction_and_precision() {
                None => 999_999,
                Some((f, fp)) => {
                    (*f * u32::pow(10, 6 - u32::from(*fp))) + (u32::pow(10, 6 - u32::from(*fp))) - 1
                }
            },
        );
        NaiveTime::from_hms_micro_opt((*h).into(), (*m).into(), (*s).into(), f).context(InvalidTimeSnafu)
    }
    fn range(&self) -> Result<TimeRange> {
        let start = self.earliest()?;
        let end = self.latest()?;
        TimeRange::from_start_to_end(start, end)
    }
}

impl AsRange for DicomDateTime {
    type Item = DateTime<FixedOffset>;
    type Range = DateTimeRange;
    fn earliest(&self) -> Result<DateTime<FixedOffset>> {
        let date = self.date().earliest()?;
        let time = match self.time() {
            Some(time) => time.earliest()?,
            None => NaiveTime::from_hms(0, 0, 0),
        };

        self.offset()
            .from_utc_date(&date)
            .and_time(time)
            .context(InvalidDateTimeSnafu)
    }

    fn latest(&self) -> Result<DateTime<FixedOffset>> {
        let date = self.date().latest()?;
        let time = match self.time() {
            Some(time) => time.latest()?,
            None => NaiveTime::from_hms_micro(23, 59, 59, 999_999),
        };
        self.offset()
            .from_utc_date(&date)
            .and_time(time)
            .context(InvalidDateTimeSnafu)
    }
    fn range(&self) -> Result<DateTimeRange> {
        let start = self.earliest()?;
        let end = self.latest()?;
        DateTimeRange::from_start_to_end(start, end)
    }
}

impl DicomDate {
    /// Retrieves a `chrono::NaiveDate`
    /// if the value is precise up to the day of the month.
    pub fn to_naive_date(self) -> Result<NaiveDate> {
        self.exact()
    }
}

impl DicomTime {
    
    /// Retrieves a `chrono::NaiveTime`
    /// if the value is precise up to the second.
    /// 
    /// Missing second fraction defaults to zero.
    pub fn to_naive_time(self) -> Result<NaiveTime> {
        match self.precision() {
            DateComponent::Second | DateComponent::Fraction => self.earliest(),
            _ => ImpreciseValueSnafu.fail(),
        }
    }
}

impl DicomDateTime {
    /// Retrieves a `chrono::DateTime<FixedOffset>` if value is precise.
    pub fn to_chrono_datetime(self) -> Result<DateTime<FixedOffset>> {
        // tweak here, if full DicomTime precision req. proves impractical
        self.exact()
    }
}

/// Represents a date range as two `Option<chrono::NaiveDate>` values.
/// `None` means no upper or no lower bound for range is present.
/// # Example
/// ```
/// use chrono::NaiveDate;
/// use dicom_core::value::DateRange;
///
/// let dr = DateRange::from_start(NaiveDate::from_ymd(2000, 5, 3));
///
/// assert!(dr.start().is_some());
/// assert!(dr.end().is_none());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DateRange {
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
}
/// Represents a time range as two `Option<chrono::NaiveTime>` values.
/// `None` means no upper or no lower bound for range is present.
/// # Example
/// ```
/// use chrono::NaiveTime;
/// use dicom_core::value::TimeRange;
///
/// let tr = TimeRange::from_end(NaiveTime::from_hms(10, 30, 15));
///
/// assert!(tr.start().is_none());
/// assert!(tr.end().is_some());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
}
/// Represents a date-time range as two `Option<chrono::DateTime<FixedOffset>>` values.
/// `None` means no upper or no lower bound for range is present.
/// # Example
/// ```
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// use chrono::{DateTime, FixedOffset, TimeZone};
/// use dicom_core::value::DateTimeRange;
///
/// let offset = FixedOffset::west(3600);
///
/// let dtr = DateTimeRange::from_start_to_end(
///     offset.ymd(2000, 5, 6).and_hms(15, 0, 0),
///     offset.ymd(2000, 5, 6).and_hms(16, 30, 0)
/// )?;
///
/// assert!(dtr.start().is_some());
/// assert!(dtr.end().is_some());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DateTimeRange {
    start: Option<DateTime<FixedOffset>>,
    end: Option<DateTime<FixedOffset>>,
}

impl DateRange {

    /// Constructs a new `DateRange` from two `chrono::NaiveDate` values
    /// monotonically ordered in time.
    pub fn from_start_to_end(start: NaiveDate, end: NaiveDate) -> Result<DateRange> {
        if start > end {
            RangeInversionSnafu {
                start: start.to_string(),
                end: end.to_string(),
            }
            .fail()
        } else {
            Ok(DateRange {
                start: Some(start),
                end: Some(end),
            })
        }
    }

    /// Constructs a new `DateRange` beginning with a `chrono::NaiveDate` value
    /// and no upper limit.
    pub fn from_start(start: NaiveDate) -> DateRange {
        DateRange {
            start: Some(start),
            end: None,
        }
    }

    /// Constructs a new `DateRange` with no lower limit, ending with a `chrono::NaiveDate` value.
    pub fn from_end(end: NaiveDate) -> DateRange {
        DateRange {
            start: None,
            end: Some(end),
        }
    }

    /// Returns a reference to lower bound of range.
    pub fn start(&self) -> Option<&NaiveDate> {
        self.start.as_ref()
    }

    /// Returns a reference to upper bound of range.
    pub fn end(&self) -> Option<&NaiveDate> {
        self.end.as_ref()
    }
}

impl TimeRange {

    /// Constructs a new `TimeRange` from two `chrono::NaiveTime` values
    /// monotonically ordered in time.
    pub fn from_start_to_end(start: NaiveTime, end: NaiveTime) -> Result<TimeRange> {
        if start > end {
            RangeInversionSnafu {
                start: start.to_string(),
                end: end.to_string(),
            }
            .fail()
        } else {
            Ok(TimeRange {
                start: Some(start),
                end: Some(end),
            })
        }
    }

    /// Constructs a new `TimeRange` beginning with a `chrono::NaiveTime` value
    /// and no upper limit.
    pub fn from_start(start: NaiveTime) -> TimeRange {
        TimeRange {
            start: Some(start),
            end: None,
        }
    }

    /// Constructs a new `TimeRange` with no lower limit, ending with a `chrono::NaiveTime` value.
    pub fn from_end(end: NaiveTime) -> TimeRange {
        TimeRange {
            start: None,
            end: Some(end),
        }
    }

    /// Returns a reference to the lower bound of the range.
    pub fn start(&self) -> Option<&NaiveTime> {
        self.start.as_ref()
    }

    /// Returns a reference to the upper bound of the range.
    pub fn end(&self) -> Option<&NaiveTime> {
        self.end.as_ref()
    }
}

impl DateTimeRange {

    /// Constructs a new `DateTimeRange` from two `chrono::DateTime` values
    /// monotonically ordered in time.
    pub fn from_start_to_end(
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> Result<DateTimeRange> {
        if start > end {
            RangeInversionSnafu {
                start: start.to_string(),
                end: end.to_string(),
            }
            .fail()
        } else {
            Ok(DateTimeRange {
                start: Some(start),
                end: Some(end),
            })
        }
    }

    /// Constructs a new `DateTimeRange` beginning with a `chrono::DateTime` value
    /// and no upper limit.
    pub fn from_start(start: DateTime<FixedOffset>) -> DateTimeRange {
        DateTimeRange {
            start: Some(start),
            end: None,
        }
    }

    /// Constructs a new `DateTimeRange` with no lower limit, ending with a `chrono::DateTime` value.
    pub fn from_end(end: DateTime<FixedOffset>) -> DateTimeRange {
        DateTimeRange {
            start: None,
            end: Some(end),
        }
    }

    /// Returns a reference to the lower bound of the range.
    pub fn start(&self) -> Option<&DateTime<FixedOffset>> {
        self.start.as_ref()
    }

    /// Returns a reference to the upper bound of the range.
    pub fn end(&self) -> Option<&DateTime<FixedOffset>> {
        self.end.as_ref()
    }

    /// For combined datetime range matching,
    /// this method constructs a `DateTimeRange` from a `DateRange` and a `TimeRange`.
    pub fn from_date_and_time_range(
        dr: DateRange,
        tr: TimeRange,
        offset: FixedOffset,
    ) -> Result<DateTimeRange> {
        let start_date = dr.start();
        let end_date = dr.end();

        let start_time = *tr.start().unwrap_or(&NaiveTime::from_hms(0, 0, 0));
        let end_time = *tr
            .end()
            .unwrap_or(&NaiveTime::from_hms_micro(23, 59, 59, 999_999));

        match start_date {
            Some(sd) => match end_date {
                Some(ed) => Ok(DateTimeRange::from_start_to_end(
                    offset
                        .from_utc_date(sd)
                        .and_time(start_time)
                        .context(InvalidDateTimeSnafu)?,
                    offset
                        .from_utc_date(ed)
                        .and_time(end_time)
                        .context(InvalidDateTimeSnafu)?,
                )?),
                None => Ok(DateTimeRange::from_start(
                    offset
                        .from_utc_date(sd)
                        .and_time(start_time)
                        .context(InvalidDateTimeSnafu)?,
                )),
            },
            None => match end_date {
                Some(ed) => Ok(DateTimeRange::from_end(
                    offset
                        .from_utc_date(ed)
                        .and_time(end_time)
                        .context(InvalidDateTimeSnafu)?,
                )),
                None => panic!("Impossible combination of two None values for a date range."),
            },
        }
    }
}

/**
 *  Looks for a range separator '-'.
 *  Returns a `DateRange`.
 */
pub fn parse_date_range(buf: &[u8]) -> Result<DateRange> {
    // minimum length of one valid DicomDate (YYYY) and one '-' separator
    if buf.len() < 5 {
        return UnexpectedEndOfElementSnafu.fail();
    }

    if let Some(separator) = buf.iter().position(|e| *e == b'-') {
        let (start, end) = buf.split_at(separator);
        let end = &end[1..];
        match separator {
            0 => Ok(DateRange::from_end(
                parse_date_partial(end).context(ParseSnafu)?.0.latest()?,
            )),
            i if i == buf.len() - 1 => Ok(DateRange::from_start(
                parse_date_partial(start).context(ParseSnafu)?.0.earliest()?,
            )),
            _ => Ok(DateRange::from_start_to_end(
                parse_date_partial(start).context(ParseSnafu)?.0.earliest()?,
                parse_date_partial(end).context(ParseSnafu)?.0.latest()?,
            )?),
        }
    } else {
        NoRangeSeparatorSnafu.fail()
    }
}

/// Looks for a range separator '-'.
///  Returns a `TimeRange`.
pub fn parse_time_range(buf: &[u8]) -> Result<TimeRange> {
    // minimum length of one valid DicomTime (HH) and one '-' separator
    if buf.len() < 3 {
        return UnexpectedEndOfElementSnafu.fail();
    }

    if let Some(separator) = buf.iter().position(|e| *e == b'-') {
        let (start, end) = buf.split_at(separator);
        let end = &end[1..];
        match separator {
            0 => Ok(TimeRange::from_end(
                parse_time_partial(end).context(ParseSnafu)?.0.latest()?,
            )),
            i if i == buf.len() - 1 => Ok(TimeRange::from_start(
                parse_time_partial(start).context(ParseSnafu)?.0.earliest()?,
            )),
            _ => Ok(TimeRange::from_start_to_end(
                parse_time_partial(start).context(ParseSnafu)?.0.earliest()?,
                parse_time_partial(end).context(ParseSnafu)?.0.latest()?,
            )?),
        }
    } else {
        NoRangeSeparatorSnafu.fail()
    }
}


/// Looks for a range separator '-'.
/// Returns a `DateTimeRange`.
/// Users are advised, that for very specific inputs, inconsistent behavior can occur.
/// This behavior can only be produced when all of the following is true:
/// - two very short date-times in the form of YYYY are presented
/// - both YYYY values can be exchanged for a valid west UTC offset, meaning year <= 1200
/// - only one west UTC offset is presented.
/// In such cases, two '-' characters are present and the parser will favor the first one,
/// if it produces a valid `DateTimeRange`. Otherwise, it tries the second one.
pub fn parse_datetime_range(buf: &[u8], dt_utc_offset: FixedOffset) -> Result<DateTimeRange> {
    // minimum length of one valid DicomDateTime (YYYY) and one '-' separator
    if buf.len() < 5 {
        return UnexpectedEndOfElementSnafu.fail();
    }
    // simplest first, check for open upper and lower bound of range
    if buf[0] == b'-' {
        // starting with separator, range is None-Some
        let buf = &buf[1..];
        Ok(DateTimeRange::from_end(
            parse_datetime_partial(buf, dt_utc_offset)
                .context(ParseSnafu)?
                .latest()?,
        ))
    } else if buf[buf.len() - 1] == b'-' {
        // ends with separator, range is Some-None
        let buf = &buf[0..(buf.len() - 1)];
        Ok(DateTimeRange::from_start(
            parse_datetime_partial(buf, dt_utc_offset)
                .context(ParseSnafu)?
                .earliest()?,
        ))
    } else {
        // range must be Some-Some, now, count number of dashes and get their indexes
        let dashes: Vec<usize> = buf
            .iter()
            .enumerate()
            .filter(|(_i, c)| **c == b'-')
            .map(|(i, _c)| i)
            .collect();

        let separator = match dashes.len() {
            0 => return NoRangeSeparatorSnafu.fail(), // no separator
            1 => dashes[0],                      // the only possible separator
            2 => {
                // there's one West UTC offset (-hhmm) in one part of the range
                let (start1, end1) = buf.split_at(dashes[0]);

                let first = (
                    parse_datetime_partial(start1, dt_utc_offset),
                    parse_datetime_partial(&end1[1..], dt_utc_offset),
                );
                match first {
                    // if split at the first dash produces a valid range, accept. Else try the other dash
                    (Ok(s), Ok(e)) => {
                        //create a result here, to check for range inversion
                        let dtr = DateTimeRange::from_start_to_end(s.earliest()?, e.latest()?);
                        match dtr {
                            Ok(val) => return Ok(val),
                            Err(_) => dashes[1],
                        }
                    }
                    _ => dashes[1],
                }
            }
            3 => dashes[1], // maximum valid count of dashes, two West UTC offsets and one separator, it's middle one
            len => return SeparatorCountSnafu { value: len }.fail(),
        };

        let (start, end) = buf.split_at(separator);
        let end = &end[1..];
        DateTimeRange::from_start_to_end(
            parse_datetime_partial(start, dt_utc_offset)
                .context(ParseSnafu)?
                .earliest()?,
            parse_datetime_partial(end, dt_utc_offset)
                .context(ParseSnafu)?
                .latest()?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_range() {
        assert_eq!(
            DateRange::from_start(NaiveDate::from_ymd(2020, 1, 1)).start(),
            Some(&NaiveDate::from_ymd(2020, 1, 1))
        );
        assert_eq!(
            DateRange::from_end(NaiveDate::from_ymd(2020, 12, 31)).end(),
            Some(&NaiveDate::from_ymd(2020, 12, 31))
        );
        assert_eq!(
            DateRange::from_start_to_end(
                NaiveDate::from_ymd(2020, 1, 1),
                NaiveDate::from_ymd(2020, 12, 31)
            )
            .unwrap()
            .start(),
            Some(&NaiveDate::from_ymd(2020, 1, 1))
        );
        assert_eq!(
            DateRange::from_start_to_end(
                NaiveDate::from_ymd(2020, 1, 1),
                NaiveDate::from_ymd(2020, 12, 31)
            )
            .unwrap()
            .end(),
            Some(&NaiveDate::from_ymd(2020, 12, 31))
        );
        assert!(matches!(
            DateRange::from_start_to_end(
                NaiveDate::from_ymd(2020, 12, 1),
                NaiveDate::from_ymd(2020, 1, 1)
            ),
            Err(Error::RangeInversion {
                start, end ,.. }) if start == "2020-12-01" && end == "2020-01-01"
        ));
    }

    #[test]
    fn test_time_range() {
        assert_eq!(
            TimeRange::from_start(NaiveTime::from_hms(05, 05, 05)).start(),
            Some(&NaiveTime::from_hms(05, 05, 05))
        );
        assert_eq!(
            TimeRange::from_end(NaiveTime::from_hms(05, 05, 05)).end(),
            Some(&NaiveTime::from_hms(05, 05, 05))
        );
        assert_eq!(
            TimeRange::from_start_to_end(
                NaiveTime::from_hms(05, 05, 05),
                NaiveTime::from_hms(05, 05, 06)
            )
            .unwrap()
            .start(),
            Some(&NaiveTime::from_hms(05, 05, 05))
        );
        assert_eq!(
            TimeRange::from_start_to_end(
                NaiveTime::from_hms(05, 05, 05),
                NaiveTime::from_hms(05, 05, 06)
            )
            .unwrap()
            .end(),
            Some(&NaiveTime::from_hms(05, 05, 06))
        );
        assert!(matches!(
            TimeRange::from_start_to_end(
                NaiveTime::from_hms_micro(05, 05, 05, 123_456),
                NaiveTime::from_hms_micro(05, 05, 05, 123_450)
            ),
            Err(Error::RangeInversion {
                start, end ,.. }) if start == "05:05:05.123456" && end == "05:05:05.123450"
        ));
    }

    #[test]
    fn test_datetime_range() {
        let offset = FixedOffset::west(3600);

        assert_eq!(
            DateTimeRange::from_start(offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 1)).start(),
            Some(&offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 1))
        );
        assert_eq!(
            DateTimeRange::from_end(offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 1)).end(),
            Some(&offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 1))
        );
        assert_eq!(
            DateTimeRange::from_start_to_end(
                offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 1),
                offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 5)
            )
            .unwrap()
            .start(),
            Some(&offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 1))
        );
        assert_eq!(
            DateTimeRange::from_start_to_end(
                offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 1),
                offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 5)
            )
            .unwrap()
            .end(),
            Some(&offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 5))
        );
        assert!(matches!(
            DateTimeRange::from_start_to_end(
                offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 5),
                offset.ymd(1990, 1, 1).and_hms_micro(1, 1, 1, 1)
            ),
            Err(Error::RangeInversion {
                start, end ,.. })
                if start == "1990-01-01 01:01:01.000005 -01:00" &&
                   end == "1990-01-01 01:01:01.000001 -01:00"
        ));
    }

    #[test]
    fn test_parse_date_range() {
        assert_eq!(
            parse_date_range(b"-19900201").ok(),
            Some(DateRange {
                start: None,
                end: Some(NaiveDate::from_ymd(1990, 2, 1))
            })
        );
        assert_eq!(
            parse_date_range(b"-202002").ok(),
            Some(DateRange {
                start: None,
                end: Some(NaiveDate::from_ymd(2020, 2, 29))
            })
        );
        assert_eq!(
            parse_date_range(b"-0020").ok(),
            Some(DateRange {
                start: None,
                end: Some(NaiveDate::from_ymd(20, 12, 31))
            })
        );
        assert_eq!(
            parse_date_range(b"0002-").ok(),
            Some(DateRange {
                start: Some(NaiveDate::from_ymd(2, 1, 1)),
                end: None
            })
        );
        assert_eq!(
            parse_date_range(b"000203-").ok(),
            Some(DateRange {
                start: Some(NaiveDate::from_ymd(2, 3, 1)),
                end: None
            })
        );
        assert_eq!(
            parse_date_range(b"00020307-").ok(),
            Some(DateRange {
                start: Some(NaiveDate::from_ymd(2, 3, 7)),
                end: None
            })
        );
        assert_eq!(
            parse_date_range(b"0002-202002  ").ok(),
            Some(DateRange {
                start: Some(NaiveDate::from_ymd(2, 1, 1)),
                end: Some(NaiveDate::from_ymd(2020, 2, 29))
            })
        );
        assert!(parse_date_range(b"0002").is_err());
        assert!(parse_date_range(b"0002x").is_err());
        assert!(parse_date_range(b" 2010-2020").is_err());
    }

    #[test]
    fn test_parse_time_range() {
        assert_eq!(
            parse_time_range(b"-101010.123456789").ok(),
            Some(TimeRange {
                start: None,
                end: Some(NaiveTime::from_hms_micro(10, 10, 10, 123_456))
            })
        );
        assert_eq!(
            parse_time_range(b"-101010.123 ").ok(),
            Some(TimeRange {
                start: None,
                end: Some(NaiveTime::from_hms_micro(10, 10, 10, 123_999))
            })
        );
        assert_eq!(
            parse_time_range(b"-01 ").ok(),
            Some(TimeRange {
                start: None,
                end: Some(NaiveTime::from_hms_micro(01, 59, 59, 999_999))
            })
        );
        assert_eq!(
            parse_time_range(b"101010.123456-").ok(),
            Some(TimeRange {
                start: Some(NaiveTime::from_hms_micro(10, 10, 10, 123_456)),
                end: None
            })
        );
        assert_eq!(
            parse_time_range(b"101010.123-").ok(),
            Some(TimeRange {
                start: Some(NaiveTime::from_hms_micro(10, 10, 10, 123_000)),
                end: None
            })
        );
        assert_eq!(
            parse_time_range(b"1010-").ok(),
            Some(TimeRange {
                start: Some(NaiveTime::from_hms(10, 10, 0)),
                end: None
            })
        );
        assert_eq!(
            parse_time_range(b"00-").ok(),
            Some(TimeRange {
                start: Some(NaiveTime::from_hms(0, 0, 0)),
                end: None
            })
        );
    }

    #[test]
    fn test_parse_datetime_range() {
        let offset = FixedOffset::west(3600);
        assert_eq!(
            parse_datetime_range(b"-20200229153420.123456", offset).ok(),
            Some(DateTimeRange {
                start: None,
                end: Some(offset.ymd(2020, 2, 29).and_hms_micro(15, 34, 20, 123_456))
            })
        );
        assert_eq!(
            parse_datetime_range(b"-20200229153420.123", offset).ok(),
            Some(DateTimeRange {
                start: None,
                end: Some(offset.ymd(2020, 2, 29).and_hms_micro(15, 34, 20, 123_999))
            })
        );
        assert_eq!(
            parse_datetime_range(b"-20200229153420", offset).ok(),
            Some(DateTimeRange {
                start: None,
                end: Some(offset.ymd(2020, 2, 29).and_hms_micro(15, 34, 20, 999_999))
            })
        );
        assert_eq!(
            parse_datetime_range(b"-2020022915", offset).ok(),
            Some(DateTimeRange {
                start: None,
                end: Some(offset.ymd(2020, 2, 29).and_hms_micro(15, 59, 59, 999_999))
            })
        );
        assert_eq!(
            parse_datetime_range(b"-202002", offset).ok(),
            Some(DateTimeRange {
                start: None,
                end: Some(offset.ymd(2020, 2, 29).and_hms_micro(23, 59, 59, 999_999))
            })
        );
        assert_eq!(
            parse_datetime_range(b"0002-", offset).ok(),
            Some(DateTimeRange {
                start: Some(offset.ymd(2, 1, 1).and_hms_micro(0, 0, 0, 0)),
                end: None
            })
        );
        assert_eq!(
            parse_datetime_range(b"00021231-", offset).ok(),
            Some(DateTimeRange {
                start: Some(offset.ymd(2, 12, 31).and_hms_micro(0, 0, 0, 0)),
                end: None
            })
        );
        // two 'east' UTC offsets get parsed
        assert_eq!(
            parse_datetime_range(b"19900101+0500-1999+1400", offset).ok(),
            Some(DateTimeRange {
                start: Some(
                    FixedOffset::east(5 * 3600)
                        .ymd(1990, 1, 1)
                        .and_hms_micro(0, 0, 0, 0)
                ),
                end: Some(
                    FixedOffset::east(14 * 3600)
                        .ymd(1999, 12, 31)
                        .and_hms_micro(23, 59, 59, 999_999)
                )
            })
        );
        // two 'west' UTC offsets get parsed
        assert_eq!(
            parse_datetime_range(b"19900101-0500-1999-1200", offset).ok(),
            Some(DateTimeRange {
                start: Some(
                    FixedOffset::west(5 * 3600)
                        .ymd(1990, 1, 1)
                        .and_hms_micro(0, 0, 0, 0)
                ),
                end: Some(
                    FixedOffset::west(12 * 3600)
                        .ymd(1999, 12, 31)
                        .and_hms_micro(23, 59, 59, 999_999)
                )
            })
        );
        // 'east' and 'west' UTC offsets get parsed
        assert_eq!(
            parse_datetime_range(b"19900101+1400-1999-1200", offset).ok(),
            Some(DateTimeRange {
                start: Some(
                    FixedOffset::east(14 * 3600)
                        .ymd(1990, 1, 1)
                        .and_hms_micro(0, 0, 0, 0)
                ),
                end: Some(
                    FixedOffset::west(12 * 3600)
                        .ymd(1999, 12, 31)
                        .and_hms_micro(23, 59, 59, 999_999)
                )
            })
        );
        // one 'west' UTC offsets gets parsed, offset cannot be mistaken for a date-time
        assert_eq!(
            parse_datetime_range(b"19900101-1200-1999", offset).unwrap(),
            DateTimeRange {
                start: Some(
                    FixedOffset::west(12 * 3600)
                        .ymd(1990, 1, 1)
                        .and_hms_micro(0, 0, 0, 0)
                ),
                end: Some(offset.ymd(1999, 12, 31).and_hms_micro(23, 59, 59, 999_999))
            }
        );
        // '0500' can either be a valid west UTC offset on left side, or a valid datime on the right side
        // Now, the first dash is considered to be a separator.
        assert_eq!(
            parse_datetime_range(b"0050-0500-1000", offset).unwrap(),
            DateTimeRange {
                start: Some(offset.ymd(50, 1, 1).and_hms_micro(0, 0, 0, 0)),
                end: Some(
                    FixedOffset::west(10 * 3600)
                        .ymd(500, 12, 31)
                        .and_hms_micro(23, 59, 59, 999_999)
                )
            }
        );
        // sequence with more than 3 dashes '-' is refused.
        assert!(matches!(
            parse_datetime_range(b"0001-00021231-2021-0100-0100", offset),
            Err(Error::SeparatorCount { .. })
        ));
        // any sequence without a dash '-' is refused.
        assert!(matches!(
            parse_datetime_range(b"00021231+0500", offset),
            Err(Error::NoRangeSeparator { .. })
        ));
    }
}
