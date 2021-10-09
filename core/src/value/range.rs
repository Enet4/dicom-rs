//! Handling of date, time, date-time ranges. Needed for range matching.
//! Parsing into ranges happens via partial precision  structures (DicomDate, DicomTime,
//! DicomDatime) so ranges can handle null components in date, time, date-time values.
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};

use crate::value::deserialize::{
    parse_date_partial, parse_datetime_partial_ignore_offset, parse_time_partial,
    Error as DeserializeError,
};
use crate::value::partial::{AsRange, Error as PartialValuesError};

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
    #[snafu(display("Operation on partial value as a range failed"))]
    PartialAsRange {
        #[snafu(backtrace)]
        source: PartialValuesError,
    },
    #[snafu(display("End {} is before start {}", end, start))]
    RangeInversion {
        start: String,
        end: String,
        backtrace: Backtrace,
    },
    #[snafu(display("No range separator present"))]
    NoRangeSeparator { backtrace: Backtrace },
    #[snafu(display("For a date-time range, only a single '-' character can be present"))]
    OneRangeSeparator { backtrace: Backtrace },
    #[snafu(display("Invalid date-time"))]
    InvalidDateTime { backtrace: Backtrace },
}
type Result<T, E = Error> = std::result::Result<T, E>;

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
    /**
     * Constructs a new `DateRange` from two `chrono::NaiveDate` values
     * monotonically ordered in time.
     */
    pub fn from_start_to_end(start: NaiveDate, end: NaiveDate) -> Result<DateRange> {
        if start > end {
            RangeInversion {
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
    /**
     * Constructs a new `DateRange` beginning with a `chrono::NaiveDate` value
     * and no upper limit.
     */
    pub fn from_start(start: NaiveDate) -> DateRange {
        DateRange {
            start: Some(start),
            end: None,
        }
    }
    /**
     * Constructs a new `DateRange` with no lower limit, ending with a `chrono::NaiveDate` value.
     */
    pub fn from_end(end: NaiveDate) -> DateRange {
        DateRange {
            start: None,
            end: Some(end),
        }
    }
    /**
     * Returns a reference to lower bound of range.
     */
    pub fn start(&self) -> Option<&NaiveDate> {
        self.start.as_ref()
    }
    /**
     * Returns a reference to upper bound of range.
     */
    pub fn end(&self) -> Option<&NaiveDate> {
        self.end.as_ref()
    }
}

impl TimeRange {
    /**
     * Constructs a new `TimeRange` from two `chrono::NaiveTime` values
     * monotonically ordered in time.
     */
    pub fn from_start_to_end(start: NaiveTime, end: NaiveTime) -> Result<TimeRange> {
        if start > end {
            RangeInversion {
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
    /**
     * Constructs a new `TimeRange` beginning with a `chrono::NaiveTime` value
     * and no upper limit.
     */
    pub fn from_start(start: NaiveTime) -> TimeRange {
        TimeRange {
            start: Some(start),
            end: None,
        }
    }
    /**
     * Constructs a new `TimeRange` with no lower limit, ending with a `chrono::NaiveTime` value.
     */
    pub fn from_end(end: NaiveTime) -> TimeRange {
        TimeRange {
            start: None,
            end: Some(end),
        }
    }
    /**
     * Returns a reference to lower bound of range.
     */
    pub fn start(&self) -> Option<&NaiveTime> {
        self.start.as_ref()
    }
    /**
     * Returns a reference to upper bound of range.
     */
    pub fn end(&self) -> Option<&NaiveTime> {
        self.end.as_ref()
    }
}

impl DateTimeRange {
    /**
     * Constructs a new `DateTimeRange` from two `chrono::DateTime` values
     * monotonically ordered in time.
     */
    pub fn from_start_to_end(
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> Result<DateTimeRange> {
        if start > end {
            RangeInversion {
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
    /**
     * Constructs a new `DateTimeRange` beginning with a `chrono::DateTime` value
     * and no upper limit.
     */
    pub fn from_start(start: DateTime<FixedOffset>) -> DateTimeRange {
        DateTimeRange {
            start: Some(start),
            end: None,
        }
    }
    /**
     * Constructs a new `DateTimeRange` with no lower limit, ending with a `chrono::DateTime` value.
     */
    pub fn from_end(end: DateTime<FixedOffset>) -> DateTimeRange {
        DateTimeRange {
            start: None,
            end: Some(end),
        }
    }
    /**
     * Returns a reference to lower bound of range.
     */
    pub fn start(&self) -> Option<&DateTime<FixedOffset>> {
        self.start.as_ref()
    }
    /**
     * Returns a reference to upper bound of range.
     */
    pub fn end(&self) -> Option<&DateTime<FixedOffset>> {
        self.end.as_ref()
    }
    /**
     * For combined datetime range matching, this method constructs a `DateTimeRange` from a `DateRange` and a `TimeRange`.
     */
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
                        .context(InvalidDateTime)?,
                    offset
                        .from_utc_date(ed)
                        .and_time(end_time)
                        .context(InvalidDateTime)?,
                )?),
                None => Ok(DateTimeRange::from_start(
                    offset
                        .from_utc_date(sd)
                        .and_time(start_time)
                        .context(InvalidDateTime)?,
                )),
            },
            None => match end_date {
                Some(ed) => Ok(DateTimeRange::from_end(
                    offset
                        .from_utc_date(ed)
                        .and_time(end_time)
                        .context(InvalidDateTime)?,
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
        return UnexpectedEndOfElement.fail();
    }

    if let Some(separator) = buf.iter().position(|e| *e == b'-') {
        let (start, end) = buf.split_at(separator);
        let end = &end[1..];
        match separator {
            0 => Ok(DateRange::from_end(
                parse_date_partial(end)
                    .context(Parse)?
                    .0
                    .latest()
                    .context(PartialAsRange)?,
            )),
            i if i == buf.len() - 1 => Ok(DateRange::from_start(
                parse_date_partial(start)
                    .context(Parse)?
                    .0
                    .earliest()
                    .context(PartialAsRange)?,
            )),
            _ => Ok(DateRange::from_start_to_end(
                parse_date_partial(start)
                    .context(Parse)?
                    .0
                    .earliest()
                    .context(PartialAsRange)?,
                parse_date_partial(end)
                    .context(Parse)?
                    .0
                    .latest()
                    .context(PartialAsRange)?,
            )?),
        }
    } else {
        NoRangeSeparator.fail()
    }
}

/**
 *  Looks for a range separator '-'.
 *  Returns a `TimeRange`.
 */
pub fn parse_time_range(buf: &[u8]) -> Result<TimeRange> {
    // minimum length of one valid DicomTime (HH) and one '-' separator
    if buf.len() < 3 {
        return UnexpectedEndOfElement.fail();
    }

    if let Some(separator) = buf.iter().position(|e| *e == b'-') {
        let (start, end) = buf.split_at(separator);
        let end = &end[1..];
        match separator {
            0 => Ok(TimeRange::from_end(
                parse_time_partial(end)
                    .context(Parse)?
                    .0
                    .latest()
                    .context(PartialAsRange)?,
            )),
            i if i == buf.len() - 1 => Ok(TimeRange::from_start(
                parse_time_partial(start)
                    .context(Parse)?
                    .0
                    .earliest()
                    .context(PartialAsRange)?,
            )),
            _ => Ok(TimeRange::from_start_to_end(
                parse_time_partial(start)
                    .context(Parse)?
                    .0
                    .earliest()
                    .context(PartialAsRange)?,
                parse_time_partial(end)
                    .context(Parse)?
                    .0
                    .latest()
                    .context(PartialAsRange)?,
            )?),
        }
    } else {
        NoRangeSeparator.fail()
    }
}

/**
 *  Looks for a range separator '-'.
 *  Returns a `DateTimeRange`.
 *  As it is programatically impossible to distinguish between the shortest valid date-time
 * (-YYYY) and a valid 'west' UTC offset (-hhmm), this method assumes no offsets will be parsed and will always
 * use the provided `FixedOffset` for both values in range, effectively looking for exactly one
 * '-' character as a range separator.
 */

pub fn parse_datetime_range(buf: &[u8], dt_utc_offset: FixedOffset) -> Result<DateTimeRange> {
    // minimum length of one valid DicomDateTime (YYYY) and one '-' separator
    if buf.len() < 5 {
        return UnexpectedEndOfElement.fail();
    }
    #[allow(clippy::naive_bytecount)]
    match buf.iter().filter(|c| **c == b'-').count() {
        0 => NoRangeSeparator.fail(),
        1 => {
            let separator = buf.iter().position(|e| *e == b'-').unwrap();
            let (start, end) = buf.split_at(separator);
            let end = &end[1..];
            match separator {
                0 => Ok(DateTimeRange::from_end(
                    parse_datetime_partial_ignore_offset(end, dt_utc_offset)
                        .context(Parse)?
                        .latest()
                        .context(PartialAsRange)?,
                )),
                i if i == buf.len() - 1 => Ok(DateTimeRange::from_start(
                    parse_datetime_partial_ignore_offset(start, dt_utc_offset)
                        .context(Parse)?
                        .earliest()
                        .context(PartialAsRange)?,
                )),
                _ => Ok(DateTimeRange::from_start_to_end(
                    parse_datetime_partial_ignore_offset(start, dt_utc_offset)
                        .context(Parse)?
                        .earliest()
                        .context(PartialAsRange)?,
                    parse_datetime_partial_ignore_offset(end, dt_utc_offset)
                        .context(Parse)?
                        .latest()
                        .context(PartialAsRange)?,
                )?),
            }
        }
        _ => OneRangeSeparator.fail(),
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
        // presence of 'east' UTC offsets doesn't fail, but they are ignored.
        assert_eq!(
            parse_datetime_range(b"19900101+0500-1999+0700", offset).ok(),
            Some(DateTimeRange {
                start: Some(offset.ymd(1990, 1, 1).and_hms_micro(0, 0, 0, 0)),
                end: Some(offset.ymd(1999, 12, 31).and_hms_micro(23, 59, 59, 999_999))
            })
        );
        // any sequence with more than one dash '-' is refused.
        assert!(matches!(
            parse_datetime_range(b"00021231-0100-", offset),
            Err(Error::OneRangeSeparator { .. })
        ));
        // any sequence with more than one dash '-' is refused.
        assert!(matches!(
            parse_datetime_range(b"-00021231-0500", offset),
            Err(Error::OneRangeSeparator { .. })
        ));
        // any sequence with more than one dash '-' is refused.
        assert!(matches!(
            parse_datetime_range(b"1990-0100-1999-0100", offset),
            Err(Error::OneRangeSeparator { .. })
        ));
        // any sequence with more than one dash '-' is refused.
        assert!(matches!(
            parse_datetime_range(b"1990-1999-0200", offset),
            Err(Error::OneRangeSeparator { .. })
        ));
    }
}
