//! Handling of date, time, date-time ranges. Needed for range matching.
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};
use snafu::{Backtrace, ResultExt, Snafu};

use crate::value::deserialize::{
    parse_date_partial, parse_datetime_partial, parse_time_partial, Error as DeserializeError,
};
use crate::value::partial::{AsRange, Error as PartialValuesError};

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Unexpected end of element."))]
    UnexpectedEndOfElement { backtrace: Backtrace },
    #[snafu(display("Failed to parse value."))]
    Parse {
        #[snafu(backtrace)]
        source: DeserializeError,
    },
    #[snafu(display("Operation on partial value as a range failed."))]
    PartialAsRange {
        #[snafu(backtrace)]
        source: PartialValuesError,
    },
    #[snafu(display("Cannot construct range from two None values."))]
    NoRange { backtrace: Backtrace },
    #[snafu(display("End {} is before start {}", end, start))]
    RangeInversion {
        start: String,
        end: String,
        backtrace: Backtrace,
    },
    #[snafu(display("No range separator present"))]
    NoRangeSeparator { backtrace: Backtrace },
}
type Result<T, E = Error> = std::result::Result<T, E>;

/// Represents a date range in `chrono::NaiveDate`.
/// `None` means no upper or no lower bound for range is present.
#[derive(Debug, Clone, Copy)]
pub struct DateRange {
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
}
/// Represents a time range in `chrono::NaiveTime`.
/// `None` means no upper or no lower bound for range is present.
#[derive(Debug, Clone, Copy)]
pub struct TimeRange {
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
}
/// Represents a date-time range in `chrono::DateTime<FixedOffset>`.
/// `None` means no upper or no lower bound for range is present.
#[derive(Debug, Clone, Copy)]
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
    pub fn from_start(start: NaiveDate) -> Result<DateRange> {
        Ok(DateRange {
            start: Some(start),
            end: None,
        })
    }
    /**
     * Constructs a new `DateRange` with no lower limit, ending with a `chrono::NaiveDate` value.
     */
    pub fn from_end(end: NaiveDate) -> Result<DateRange> {
        Ok(DateRange {
            start: None,
            end: Some(end),
        })
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
    pub fn from_start(start: NaiveTime) -> Result<TimeRange> {
        Ok(TimeRange {
            start: Some(start),
            end: None,
        })
    }
    /**
     * Constructs a new `TimeRange` with no lower limit, ending with a `chrono::NaiveTime` value.
     */
    pub fn from_end(end: NaiveTime) -> Result<TimeRange> {
        Ok(TimeRange {
            start: None,
            end: Some(end),
        })
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
    pub fn from_start(start: DateTime<FixedOffset>) -> Result<DateTimeRange> {
        Ok(DateTimeRange {
            start: Some(start),
            end: None,
        })
    }
    /**
     * Constructs a new `DateTimeRange` with no lower limit, ending with a `chrono::DateTime` value.
     */
    pub fn from_end(end: DateTime<FixedOffset>) -> Result<DateTimeRange> {
        Ok(DateTimeRange {
            start: None,
            end: Some(end),
        })
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
            )?),
            i if i == buf.len() - 1 => Ok(DateRange::from_start(
                parse_date_partial(start)
                    .context(Parse)?
                    .0
                    .earliest()
                    .context(PartialAsRange)?,
            )?),
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
            )?),
            i if i == buf.len() - 1 => Ok(TimeRange::from_start(
                parse_time_partial(start)
                    .context(Parse)?
                    .0
                    .earliest()
                    .context(PartialAsRange)?,
            )?),
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
 *  Returns a `DateTimeRange`
 */
pub fn parse_datetime_range(buf: &[u8], dt_utc_offset: FixedOffset) -> Result<DateTimeRange> {
    // minimum length of one valid DicomDateTime (YYYY) and one '-' separator
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
                        x if *x + 6 < buf.len() => {
                            /* separator present in 5 bytes, so assume this position is an offset sign */
                            buf[x + 5] != b'-'
                        },
                        /* Only 4 bytes follow, assume this position is a separator and YYYY follows*/
                        x if *x + 5 == buf.len() => true,
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
            0 => Ok(DateTimeRange::from_end(
                parse_datetime_partial(end, dt_utc_offset).context(Parse)?.latest().context(PartialAsRange)?
            )?),
            i if i == buf.len() - 1 => Ok(DateTimeRange::from_start(
                parse_datetime_partial(start, dt_utc_offset).context(Parse)?.earliest().context(PartialAsRange)?
            )?),
            _ => {
                Ok(DateTimeRange::from_start_to_end(
                    parse_datetime_partial(start, dt_utc_offset).context(Parse)?.earliest().context(PartialAsRange)?,
                    parse_datetime_partial(end, dt_utc_offset).context(Parse)?.latest().context(PartialAsRange)?
                )?)
            }
        }
    } else {
        NoRangeSeparator.fail()
    }
}
/*
impl fmt::Display for DateRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            (Some(s), Some(e)) => write!(f, "'{} - {}'", s, e),
            (Some(s), None) => write!(f, "'{} - '", s),
            (None, Some(e)) => write!(f, "' - {}'", e),
        }
    }
}*/
