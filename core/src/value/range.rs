//! Handling of date, time, date-time ranges. Needed for range matching.
//! Parsing into ranges happens via partial precision  structures (DicomDate, DicomTime,
//! DicomDatime) so ranges can handle null components in date, time, date-time values.
use chrono::{
    offset, DateTime, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone,
};
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
    #[snafu(display("Converting a time-zone naive value '{naive}' to a time-zone '{offset}' leads to invalid date-time or ambiguous results."))]
    InvalidDateTime {
        naive: NaiveDateTime,
        offset: FixedOffset,
        backtrace: Backtrace,
    },
    #[snafu(display(
        "Cannot convert from an imprecise value. This value represents a date / time range"
    ))]
    ImpreciseValue { backtrace: Backtrace },
    #[snafu(display("Failed to construct Date from '{y}-{m}-{d}'"))]
    InvalidDate {
        y: i32,
        m: u32,
        d: u32,
        backtrace: Backtrace,
    },
    #[snafu(display("Failed to construct Time from {h}:{m}:{s}"))]
    InvalidTime {
        h: u32,
        m: u32,
        s: u32,
        backtrace: Backtrace,
    },
    #[snafu(display("Failed to construct Time from {h}:{m}:{s}:{f}"))]
    InvalidTimeMicro {
        h: u32,
        m: u32,
        s: u32,
        f: u32,
        backtrace: Backtrace,
    },
    #[snafu(display(
        "Date-time does not contain time-zone, cannot convert to time-zone aware value"
    ))]
    NoTimeZone { backtrace: Backtrace },
    #[snafu(display(
        "Failed to convert to a time-zone aware date-time value with ambiguous results: {t1}, {t2} "
    ))]
    DateTimeAmbiguous {
        t1: DateTime<FixedOffset>,
        t2: DateTime<FixedOffset>,
    },
    #[snafu(display(
        "Failed to convert to a time-zone aware date-time value: Given local time representation is invalid"
    ))]
    DateTimeInvalid { backtrace: Backtrace },
    #[snafu(display(
        "Trying to convert a time-zone aware date-time value to a time-zone unaware value"
    ))]
    DateTimeTzAware { backtrace: Backtrace },
}
type Result<T, E = Error> = std::result::Result<T, E>;

/// The DICOM protocol accepts date / time values with null components.
///
/// Imprecise values are to be handled as date / time ranges.
///
/// This trait is implemented by date / time structures with partial precision.
/// If the date / time structure is not precise, it is up to the user to call one of these
/// methods to retrieve a suitable  [`chrono`] value.
/// 
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
///     Some(dicom_date.earliest()?),
///     NaiveDate::from_ymd_opt(2010,1,1)
/// );
/// assert_eq!(
///     Some(dicom_date.latest()?),
///     NaiveDate::from_ymd_opt(2010,1,31)
/// );
///
/// let dicom_time = DicomTime::from_hm(10,0)?;
/// assert_eq!(
///     dicom_time.range()?,
///     TimeRange::from_start_to_end(NaiveTime::from_hms(10, 0, 0),
///         NaiveTime::from_hms_micro_opt(10, 0, 59, 999_999).unwrap())?
/// );
/// // only a time with 6 digits second fraction is considered precise
/// assert!(dicom_time.exact().is_err());
///
/// # Ok(())
/// # }
/// ```
pub trait AsRange: Precision {
    type PreciseValue: PartialEq + PartialOrd;
    type Range;
    /// Returns a corresponding `chrono` value, if the partial precision structure has full accuracy.
    fn exact(&self) -> Result<Self::PreciseValue> {
        if self.is_precise() {
            Ok(self.earliest()?)
        } else {
            ImpreciseValueSnafu.fail()
        }
    }

    /// Returns the earliest possible valid `chrono` value from a partial precision structure.
    /// Missing components default to 1 (days, months) or 0 (hours, minutes, ...)
    /// If structure contains invalid combination of `DateComponent`s, it fails.
    fn earliest(&self) -> Result<Self::PreciseValue>;

    /// Returns the latest possible valid `chrono` value from a partial precision structure.
    /// If structure contains invalid combination of `DateComponent`s, it fails.
    fn latest(&self) -> Result<Self::PreciseValue>;

    /// Returns a tuple of the earliest and latest possible value from a partial precision structure.
    fn range(&self) -> Result<Self::Range>;

}

impl AsRange for DicomDate {
    type PreciseValue = NaiveDate;
    type Range = DateRange;
    fn earliest(&self) -> Result<Self::PreciseValue> {
        let (y, m, d) = {
            (
                *self.year() as i32,
                *self.month().unwrap_or(&1) as u32,
                *self.day().unwrap_or(&1) as u32,
            )
        };
        NaiveDate::from_ymd_opt(y, m, d).context(InvalidDateSnafu { y, m, d })
    }

    fn latest(&self) -> Result<Self::PreciseValue> {
        let (y, m, d) = (
            self.year(),
            self.month().unwrap_or(&12),
            match self.day() {
                Some(d) => *d as u32,
                None => {
                    let y = self.year();
                    let m = self.month().unwrap_or(&12);
                    if m == &12 {
                        NaiveDate::from_ymd_opt(*y as i32 + 1, 1, 1).context(InvalidDateSnafu {
                            y: *y as i32,
                            m: 1u32,
                            d: 1u32,
                        })?
                    } else {
                        NaiveDate::from_ymd_opt(*y as i32, *m as u32 + 1, 1).context(
                            InvalidDateSnafu {
                                y: *y as i32,
                                m: *m as u32,
                                d: 1u32,
                            },
                        )?
                    }
                    .signed_duration_since(
                        NaiveDate::from_ymd_opt(*y as i32, *m as u32, 1).context(
                            InvalidDateSnafu {
                                y: *y as i32,
                                m: *m as u32,
                                d: 1u32,
                            },
                        )?,
                    )
                    .num_days() as u32
                }
            },
        );

        NaiveDate::from_ymd_opt(*y as i32, *m as u32, d).context(InvalidDateSnafu {
            y: *y as i32,
            m: *m as u32,
            d,
        })
    }

    fn range(&self) -> Result<Self::Range> {
        let start = self.earliest()?;
        let end = self.latest()?;
        DateRange::from_start_to_end(start, end)
    }
}

impl AsRange for DicomTime {
    type PreciseValue = NaiveTime;
    type Range = TimeRange;
    fn earliest(&self) -> Result<Self::PreciseValue> {
        let (h, m, s, f) = (
            self.hour(),
            self.minute().unwrap_or(&0),
            self.second().unwrap_or(&0),
            match self.fraction_and_precision() {
                None => 0,
                Some((f, fp)) => *f * u32::pow(10, 6 - <u32>::from(*fp)),
            },
        );

        NaiveTime::from_hms_micro_opt((*h).into(), (*m).into(), (*s).into(), f).context(
            InvalidTimeMicroSnafu {
                h: *h as u32,
                m: *m as u32,
                s: *s as u32,
                f,
            },
        )
    }
    fn latest(&self) -> Result<Self::PreciseValue> {
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
        NaiveTime::from_hms_micro_opt((*h).into(), (*m).into(), (*s).into(), f).context(
            InvalidTimeMicroSnafu {
                h: *h as u32,
                m: *m as u32,
                s: *s as u32,
                f,
            },
        )
    }
    fn range(&self) -> Result<Self::Range> {
        let start = self.earliest()?;
        let end = self.latest()?;
        TimeRange::from_start_to_end(start, end)
    }
}

impl AsRange for DicomDateTime {
    type PreciseValue = PreciseDateTimeResult;
    type Range = DateTimeRange;
    fn earliest(&self) -> Result<Self::PreciseValue> {
        let date = self.date().earliest()?;
        let time = match self.time() {
            Some(time) => time.earliest()?,
            None => NaiveTime::from_hms_opt(0, 0, 0).context(InvalidTimeSnafu {
                h: 0u32,
                m: 0u32,
                s: 0u32,
            })?,
        };

        match self.time_zone() {
            Some(offset) => Ok(PreciseDateTimeResult::WithTimeZone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(date, time))
                    .single()
                    .context(InvalidDateTimeSnafu {
                        naive: NaiveDateTime::new(date, time),
                        offset: *offset,
                    })?,
            )),
            None => Ok(PreciseDateTimeResult::Naive(NaiveDateTime::new(date, time))),
        }
    }

    fn latest(&self) -> Result<Self::PreciseValue> {
        let date = self.date().latest()?;
        let time = match self.time() {
            Some(time) => time.latest()?,
            None => NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).context(
                InvalidTimeMicroSnafu {
                    h: 23u32,
                    m: 59u32,
                    s: 59u32,
                    f: 999_999u32,
                },
            )?,
        };

        match self.time_zone() {
            Some(offset) => Ok(PreciseDateTimeResult::WithTimeZone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(date, time))
                    .single()
                    .context(InvalidDateTimeSnafu {
                        naive: NaiveDateTime::new(date, time),
                        offset: *offset,
                    })?,
            )),
            None => Ok(PreciseDateTimeResult::Naive(NaiveDateTime::new(date, time))),
        }
    }
    fn range(&self) -> Result<Self::Range> {
        let start = self.earliest()?;
        let end = self.latest()?;

        if start.has_time_zone() {
            let s = start.into_datetime_with_time_zone()?;
            let e = end.into_datetime_with_time_zone()?;
            Ok(DateTimeRange::WithTimeZone {
                start: Some(s),
                end: Some(e),
            })
        } else {
            let s = start.into_datetime()?;
            let e = end.into_datetime()?;
            Ok(DateTimeRange::Naive {
                start: Some(s),
                end: Some(e),
            })
        }
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
    /// Retrieves a `chrono::DateTime<FixedOffset>`.
    /// It the value does not store a time-zone or the date-time value is not precise or the conversion leads to ambiguous results,
    /// it fails.
    /// To inspect the possibly ambiguous results of this conversion, see `to_chrono_local_result`
    pub fn to_datetime_with_time_zone(self) -> Result<DateTime<FixedOffset>> {
        self.exact()?.into_datetime_with_time_zone()
    }

    /// Retrieves a `chrono::NaiveDateTime`. If the internal date-time value is not precise or
    /// it is time-zone aware, this method will fail.
    pub fn to_naive_datetime(&self) -> Result<NaiveDateTime> {
        self.exact()?.into_datetime()
    }

    /// Retrieves a `chrono::LocalResult` by converting the internal time-zone naive date-time representation
    /// to a time-zone aware representation.
    /// It the value does not store a time-zone or the date-time value is not precise, it fails.
    pub fn to_chrono_local_result(self) -> Result<LocalResult<DateTime<FixedOffset>>> {
        if !self.is_precise() {
            return ImpreciseValueSnafu.fail();
        }

        if let Some(offset) = self.time_zone() {
            let date = self.date().earliest()?;
            let time = self.time().context(ImpreciseValueSnafu)?.earliest()?;

            let naive_date_time = NaiveDateTime::new(date, time);
            Ok(offset.from_local_datetime(&naive_date_time))
        } else {
            NoTimeZoneSnafu.fail()
        }
    }
}

/// Represents a date range as two [`Option<chrono::NaiveDate>`] values.
/// [None] means no upper or no lower bound for range is present.
/// # Example
/// ```
/// use chrono::NaiveDate;
/// use dicom_core::value::DateRange;
///
/// let dr = DateRange::from_start(NaiveDate::from_ymd_opt(2000, 5, 3).unwrap());
///
/// assert!(dr.start().is_some());
/// assert!(dr.end().is_none());
/// ```
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct DateRange {
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
}
/// Represents a time range as two [`Option<chrono::NaiveTime>`] values.
/// [None] means no upper or no lower bound for range is present.
/// # Example
/// ```
/// use chrono::NaiveTime;
/// use dicom_core::value::TimeRange;
///
/// let tr = TimeRange::from_end(NaiveTime::from_hms_opt(10, 30, 15).unwrap());
///
/// assert!(tr.start().is_none());
/// assert!(tr.end().is_some());
/// ```
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct TimeRange {
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
}
/// Represents a date-time range, that can either be time-zone naive or time-zone aware. It is stored as two [`Option<chrono::DateTime<FixedOffset>>`] or
/// two [`Option<chrono::NaiveDateTime>>`] values.
/// [None] means no upper or no lower bound for range is present.
/// Please note, that this structure is guaranteed never to contain a combination of one time-zone aware and one time-zone naive value.  
/// # Example
/// ```
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// use chrono::{NaiveDate, NaiveTime, NaiveDateTime, DateTime, FixedOffset, TimeZone};
/// use dicom_core::value::DateTimeRange;
///
/// let offset = FixedOffset::west_opt(3600).unwrap();
///
/// let dtr = DateTimeRange::from_start_to_end(
///     offset.from_local_datetime(&NaiveDateTime::new(
///         NaiveDate::from_ymd_opt(2000, 5, 6).unwrap(),
///         NaiveTime::from_hms_opt(15, 0, 0).unwrap()
///     )).unwrap(),
///     offset.from_local_datetime(&NaiveDateTime::new(
///         NaiveDate::from_ymd_opt(2000, 5, 6).unwrap(),
///         NaiveTime::from_hms_opt(16, 30, 0).unwrap()
///     )).unwrap()
/// )?;
///
/// assert!(dtr.start().is_some());
/// assert!(dtr.end().is_some());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum DateTimeRange {
    Naive {
        start: Option<NaiveDateTime>,
        end: Option<NaiveDateTime>,
    },
    WithTimeZone {
        start: Option<DateTime<FixedOffset>>,
        end: Option<DateTime<FixedOffset>>,
    },
}

/// A precise date-time value, that can either be time-zone aware or time-zone unaware.
///
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, PartialOrd)]
pub enum PreciseDateTimeResult {
    Naive(NaiveDateTime),
    WithTimeZone(DateTime<FixedOffset>),
}

impl PreciseDateTimeResult {
    /// Retrieves a reference to a `chrono::DateTime<FixedOffset>` if the result is time-zone aware.
    pub fn as_datetime_with_time_zone(&self) -> Option<&DateTime<FixedOffset>> {
        match self {
            PreciseDateTimeResult::Naive(..) => None,
            PreciseDateTimeResult::WithTimeZone(value) => Some(value),
        }
    }

    /// Retrieves a reference to a `chrono::NaiveDateTime` if the result is time-zone naive.
    pub fn as_datetime(&self) -> Option<&NaiveDateTime> {
        match self {
            PreciseDateTimeResult::Naive(value) => Some(value),
            PreciseDateTimeResult::WithTimeZone(..) => None,
        }
    }

    /// Moves out a `chrono::DateTime<FixedOffset>` if the result is time-zone aware, otherwise it fails.
    pub fn into_datetime_with_time_zone(self) -> Result<DateTime<FixedOffset>> {
        match self {
            PreciseDateTimeResult::Naive(..) => NoTimeZoneSnafu.fail(),
            PreciseDateTimeResult::WithTimeZone(value) => Ok(value),
        }
    }

    /// Moves out a `chrono::NaiveDateTime` if the result is time-zone naive, otherwise it fails.
    pub fn into_datetime(self) -> Result<NaiveDateTime> {
        match self {
            PreciseDateTimeResult::Naive(value) => Ok(value),
            PreciseDateTimeResult::WithTimeZone(..) => DateTimeTzAwareSnafu.fail(),
        }
    }

    /// Returns true if result is time-zone aware.
    pub fn has_time_zone(&self) -> bool {
        match self {
            PreciseDateTimeResult::Naive(..) => false,
            PreciseDateTimeResult::WithTimeZone(..) => true,
        }
    }
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
    /// Constructs a new `DateTimeRange` from two `chrono::DateTime<FixedOffset>` values
    /// monotonically ordered in time.
    pub fn from_start_to_end_with_time_zone(
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
            Ok(DateTimeRange::WithTimeZone {
                start: Some(start),
                end: Some(end),
            })
        }
    }

    /// Constructs a new `DateTimeRange` from two `chrono::NaiveDateTime` values
    /// monotonically ordered in time.
    pub fn from_start_to_end(start: NaiveDateTime, end: NaiveDateTime) -> Result<DateTimeRange> {
        if start > end {
            RangeInversionSnafu {
                start: start.to_string(),
                end: end.to_string(),
            }
            .fail()
        } else {
            Ok(DateTimeRange::Naive {
                start: Some(start),
                end: Some(end),
            })
        }
    }

    /// Constructs a new `DateTimeRange` beginning with a `chrono::DateTime<FixedOffset>` value
    /// and no upper limit.
    pub fn from_start_with_time_zone(start: DateTime<FixedOffset>) -> DateTimeRange {
        DateTimeRange::WithTimeZone {
            start: Some(start),
            end: None,
        }
    }

    /// Constructs a new `DateTimeRange` beginning with a `chrono::NaiveDateTime` value
    /// and no upper limit.
    pub fn from_start(start: NaiveDateTime) -> DateTimeRange {
        DateTimeRange::Naive {
            start: Some(start),
            end: None,
        }
    }

    /// Constructs a new `DateTimeRange` with no lower limit, ending with a `chrono::DateTime<FixedOffset>` value.
    pub fn from_end_with_time_zone(end: DateTime<FixedOffset>) -> DateTimeRange {
        DateTimeRange::WithTimeZone {
            start: None,
            end: Some(end),
        }
    }

    /// Constructs a new `DateTimeRange` with no lower limit, ending with a `chrono::NaiveDateTime` value.
    pub fn from_end(end: NaiveDateTime) -> DateTimeRange {
        DateTimeRange::Naive {
            start: None,
            end: Some(end),
        }
    }

    /// Returns the lower bound of the range.
    pub fn start(&self) -> Option<PreciseDateTimeResult> {
        match self {
            DateTimeRange::Naive { start, .. } => start.map(|dt| PreciseDateTimeResult::Naive(dt)),
            DateTimeRange::WithTimeZone { start, .. } => {
                start.map(|dt| PreciseDateTimeResult::WithTimeZone(dt))
            }
        }
    }

    /// Returns the upper bound of the range.
    pub fn end(&self) -> Option<PreciseDateTimeResult> {
        match self {
            DateTimeRange::Naive { start, end } => end.map(|dt| PreciseDateTimeResult::Naive(dt)),
            DateTimeRange::WithTimeZone { start, end } => {
                end.map(|dt| PreciseDateTimeResult::WithTimeZone(dt))
            }
        }
    }

    /// For combined datetime range matching,
    /// this method constructs a `DateTimeRange` from a `DateRange` and a `TimeRange`.
    /// As 'DateRange' and 'TimeRange' are always time-zone unaware, the resulting DateTimeRange
    /// will always be time-zone unaware.
    pub fn from_date_and_time_range(dr: DateRange, tr: TimeRange) -> Result<DateTimeRange> {
        let start_date = dr.start();
        let end_date = dr.end();

        let start_time = *tr
            .start()
            .unwrap_or(&NaiveTime::from_hms_opt(0, 0, 0).context(InvalidTimeSnafu {
                h: 0u32,
                m: 0u32,
                s: 0u32,
            })?);
        let end_time =
            *tr.end()
                .unwrap_or(&NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).context(
                    InvalidTimeMicroSnafu {
                        h: 23u32,
                        m: 59u32,
                        s: 59u32,
                        f: 999_999u32,
                    },
                )?);

        match start_date {
            Some(sd) => match end_date {
                Some(ed) => Ok(DateTimeRange::from_start_to_end(
                    NaiveDateTime::new(*sd, start_time),
                    NaiveDateTime::new(*ed, end_time),
                )?),
                None => Ok(DateTimeRange::from_start(NaiveDateTime::new(
                    *sd, start_time,
                ))),
            },
            None => match end_date {
                Some(ed) => Ok(DateTimeRange::from_end(NaiveDateTime::new(*ed, end_time))),
                None => panic!("Impossible combination of two None values for a date range."),
            },
        }
    }

    /*pub fn has_time_zone(&self) -> bool {
        self.time_zone().is_some()
    }

    pub fn time_zone(&self) -> Option<&FixedOffset> {
        self.time_zone.as_ref()
    }*/
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
                parse_date_partial(start)
                    .context(ParseSnafu)?
                    .0
                    .earliest()?,
            )),
            _ => Ok(DateRange::from_start_to_end(
                parse_date_partial(start)
                    .context(ParseSnafu)?
                    .0
                    .earliest()?,
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
                parse_time_partial(start)
                    .context(ParseSnafu)?
                    .0
                    .earliest()?,
            )),
            _ => Ok(TimeRange::from_start_to_end(
                parse_time_partial(start)
                    .context(ParseSnafu)?
                    .0
                    .earliest()?,
                parse_time_partial(end).context(ParseSnafu)?.0.latest()?,
            )?),
        }
    } else {
        NoRangeSeparatorSnafu.fail()
    }
}

/// Looks for a range separator '-'.
/// Returns a `DateTimeRange`.
/// If the parser encounters two date-time values, where one is time-zone aware and the other is not,
/// it will automatically convert the time-zone naive value to a time-zone aware value with zero east offset.
/// (same as appending '+0000' to the text representation).
/// Users are advised, that for very specific inputs, inconsistent behavior can occur.
/// This behavior can only be produced when all of the following is true:
/// - two very short date-times in the form of YYYY are presented (YYYY-YYYY)
/// - both YYYY values can be exchanged for a valid west UTC offset, meaning year <= 1200 e.g. (1000-1100)
/// - only one west UTC offset is presented. e.g. (1000-1100-0100)
/// In such cases, two '-' characters are present and the parser will favor the first one as a range separator,
/// if it produces a valid `DateTimeRange`. Otherwise, it tries the second one.
pub fn parse_datetime_range(buf: &[u8]) -> Result<DateTimeRange> {
    // minimum length of one valid DicomDateTime (YYYY) and one '-' separator
    if buf.len() < 5 {
        return UnexpectedEndOfElementSnafu.fail();
    }
    // simplest first, check for open upper and lower bound of range
    if buf[0] == b'-' {
        // starting with separator, range is None-Some
        let buf = &buf[1..];
        match parse_datetime_partial(buf).context(ParseSnafu)?.latest()? {
            PreciseDateTimeResult::Naive(end) => Ok(DateTimeRange::from_end(end)),
            PreciseDateTimeResult::WithTimeZone(end_tz) => {
                Ok(DateTimeRange::from_end_with_time_zone(end_tz))
            }
        }
    } else if buf[buf.len() - 1] == b'-' {
        // ends with separator, range is Some-None
        let buf = &buf[0..(buf.len() - 1)];
        match parse_datetime_partial(buf)
            .context(ParseSnafu)?
            .earliest()?
        {
            PreciseDateTimeResult::Naive(start) => Ok(DateTimeRange::from_start(start)),
            PreciseDateTimeResult::WithTimeZone(start_tz) => {
                Ok(DateTimeRange::from_start_with_time_zone(start_tz))
            }
        }
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
            1 => dashes[0],                           // the only possible separator
            2 => {
                // there's one West UTC offset (-hhmm) in one part of the range
                let (start1, end1) = buf.split_at(dashes[0]);

                let first = (
                    parse_datetime_partial(start1),
                    parse_datetime_partial(&end1[1..]),
                );
                match first {
                    // if split at the first dash produces a valid range, accept. Else try the other dash
                    (Ok(s), Ok(e)) => {
                        //create a result here, to check for range inversion
                        let dtr = match (s.earliest()?, e.latest()?) {
                            (
                                PreciseDateTimeResult::Naive(start),
                                PreciseDateTimeResult::Naive(end),
                            ) => DateTimeRange::from_start_to_end(start, end),
                            (
                                PreciseDateTimeResult::WithTimeZone(start),
                                PreciseDateTimeResult::WithTimeZone(end),
                            ) => DateTimeRange::from_start_to_end_with_time_zone(start, end),
                            (
                                PreciseDateTimeResult::Naive(start),
                                PreciseDateTimeResult::WithTimeZone(end),
                            ) => {
                                let offset = FixedOffset::east_opt(0).unwrap();
                                DateTimeRange::from_start_to_end_with_time_zone(
                                    offset.from_local_datetime(&start).single().context(
                                        InvalidDateTimeSnafu {
                                            naive: start,
                                            offset: offset,
                                        },
                                    )?,
                                    end,
                                )
                            }
                            (
                                PreciseDateTimeResult::WithTimeZone(start),
                                PreciseDateTimeResult::Naive(end),
                            ) => {
                                let offset = FixedOffset::east_opt(0).unwrap();
                                DateTimeRange::from_start_to_end_with_time_zone(
                                    start,
                                    offset.from_local_datetime(&end).single().context(
                                        InvalidDateTimeSnafu {
                                            naive: end,
                                            offset: offset,
                                        },
                                    )?,
                                )
                            }
                        };
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

        match (
            parse_datetime_partial(start)
                .context(ParseSnafu)?
                .earliest()?,
            parse_datetime_partial(end).context(ParseSnafu)?.latest()?,
        ) {
            (PreciseDateTimeResult::Naive(start), PreciseDateTimeResult::Naive(end)) => {
                DateTimeRange::from_start_to_end(start, end)
            }
            (
                PreciseDateTimeResult::WithTimeZone(start),
                PreciseDateTimeResult::WithTimeZone(end),
            ) => DateTimeRange::from_start_to_end_with_time_zone(start, end),
            (PreciseDateTimeResult::Naive(start), PreciseDateTimeResult::WithTimeZone(end)) => {
                let offset = FixedOffset::east_opt(0).unwrap();
                DateTimeRange::from_start_to_end_with_time_zone(
                    offset
                        .from_local_datetime(&start)
                        .single()
                        .context(InvalidDateTimeSnafu {
                            naive: start,
                            offset: offset,
                        })?,
                    end,
                )
            }
            (PreciseDateTimeResult::WithTimeZone(start), PreciseDateTimeResult::Naive(end)) => {
                let offset = FixedOffset::east_opt(0).unwrap();
                DateTimeRange::from_start_to_end_with_time_zone(
                    start,
                    offset
                        .from_local_datetime(&end)
                        .single()
                        .context(InvalidDateTimeSnafu {
                            naive: end,
                            offset: offset,
                        })?,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_range() {
        assert_eq!(
            DateRange::from_start(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()).start(),
            Some(&NaiveDate::from_ymd_opt(2020, 1, 1).unwrap())
        );
        assert_eq!(
            DateRange::from_end(NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()).end(),
            Some(&NaiveDate::from_ymd_opt(2020, 12, 31).unwrap())
        );
        assert_eq!(
            DateRange::from_start_to_end(
                NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()
            )
            .unwrap()
            .start(),
            Some(&NaiveDate::from_ymd_opt(2020, 1, 1).unwrap())
        );
        assert_eq!(
            DateRange::from_start_to_end(
                NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()
            )
            .unwrap()
            .end(),
            Some(&NaiveDate::from_ymd_opt(2020, 12, 31).unwrap())
        );
        assert!(matches!(
            DateRange::from_start_to_end(
                NaiveDate::from_ymd_opt(2020, 12, 1).unwrap(),
                NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()
            ),
            Err(Error::RangeInversion {
                start, end ,.. }) if start == "2020-12-01" && end == "2020-01-01"
        ));
    }

    #[test]
    fn test_time_range() {
        assert_eq!(
            TimeRange::from_start(NaiveTime::from_hms_opt(05, 05, 05).unwrap()).start(),
            Some(&NaiveTime::from_hms_opt(05, 05, 05).unwrap())
        );
        assert_eq!(
            TimeRange::from_end(NaiveTime::from_hms_opt(05, 05, 05).unwrap()).end(),
            Some(&NaiveTime::from_hms_opt(05, 05, 05).unwrap())
        );
        assert_eq!(
            TimeRange::from_start_to_end(
                NaiveTime::from_hms_opt(05, 05, 05).unwrap(),
                NaiveTime::from_hms_opt(05, 05, 06).unwrap()
            )
            .unwrap()
            .start(),
            Some(&NaiveTime::from_hms_opt(05, 05, 05).unwrap())
        );
        assert_eq!(
            TimeRange::from_start_to_end(
                NaiveTime::from_hms_opt(05, 05, 05).unwrap(),
                NaiveTime::from_hms_opt(05, 05, 06).unwrap()
            )
            .unwrap()
            .end(),
            Some(&NaiveTime::from_hms_opt(05, 05, 06).unwrap())
        );
        assert!(matches!(
            TimeRange::from_start_to_end(
                NaiveTime::from_hms_micro_opt(05, 05, 05, 123_456).unwrap(),
                NaiveTime::from_hms_micro_opt(05, 05, 05, 123_450).unwrap()
            ),
            Err(Error::RangeInversion {
                start, end ,.. }) if start == "05:05:05.123456" && end == "05:05:05.123450"
        ));
    }

    #[test]
    fn test_datetime_range_with_time_zone() {
        let offset = FixedOffset::west_opt(3600).unwrap();

        assert_eq!(
            DateTimeRange::from_start_with_time_zone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ))
                    .unwrap()
            )
            .start(),
            Some(PreciseDateTimeResult::WithTimeZone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ))
                    .unwrap()
                )
            )
        );
        assert_eq!(
            DateTimeRange::from_end_with_time_zone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ))
                    .unwrap()
            )
            .end(),
            Some(PreciseDateTimeResult::WithTimeZone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ))
                    .unwrap()
                )
            )
        );
        assert_eq!(
            DateTimeRange::from_start_to_end_with_time_zone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ))
                    .unwrap(),
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 5).unwrap()
                    ))
                    .unwrap()
            )
            .unwrap()
            .start(),
            Some(PreciseDateTimeResult::WithTimeZone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ))
                    .unwrap()
            ))
        );
        assert_eq!(
            DateTimeRange::from_start_to_end_with_time_zone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ))
                    .unwrap(),
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 5).unwrap()
                    ))
                    .unwrap()
            )
            .unwrap()
            .end(),
            Some(PreciseDateTimeResult::WithTimeZone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 5).unwrap()
                    ))
                    .unwrap()
            ))
        );
        assert!(matches!(
            DateTimeRange::from_start_to_end_with_time_zone(
                offset
                .from_local_datetime(&NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                    NaiveTime::from_hms_micro_opt(1, 1, 1, 5).unwrap()
                ))
                .unwrap(),
                offset
                .from_local_datetime(&NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                    NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                ))
                .unwrap()
            )
           ,
            Err(Error::RangeInversion {
                start, end ,.. })
                if start == "1990-01-01 01:01:01.000005 -01:00" &&
                   end == "1990-01-01 01:01:01.000001 -01:00"
        ));
    }

    #[test]
    fn test_datetime_range_naive() {
        assert_eq!(
            DateTimeRange::from_start(
                NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    )
                    
            )
            .start(),
            Some(PreciseDateTimeResult::Naive(
                NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ))
                    
                
            )
        );
        assert_eq!(
            DateTimeRange::from_end(
                NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    )
            )
            .end(),
            Some(PreciseDateTimeResult::Naive(
                NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    )
                )
            )
        );
        assert_eq!(
            DateTimeRange::from_start_to_end(
                NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ),
                NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 5).unwrap()
                    )
            )
            .unwrap()
            .start(),
            Some(PreciseDateTimeResult::Naive(
                NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    )
            ))
        );
        assert_eq!(
            DateTimeRange::from_start_to_end(
                NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ),
                NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 5).unwrap()
                    )
            )
            .unwrap()
            .end(),
            Some(PreciseDateTimeResult::Naive(
                NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 5).unwrap()
                    )
            ))
        );
        assert!(matches!(
            DateTimeRange::from_start_to_end(
                NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                    NaiveTime::from_hms_micro_opt(1, 1, 1, 5).unwrap()
                ),
                NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                    NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                )
            )
           ,
            Err(Error::RangeInversion {
                start, end ,.. })
                if start == "1990-01-01 01:01:01.000005" &&
                   end == "1990-01-01 01:01:01.000001"
        ));
    }


    #[test]
    fn test_parse_date_range() {
        assert_eq!(
            parse_date_range(b"-19900201").ok(),
            Some(DateRange {
                start: None,
                end: Some(NaiveDate::from_ymd_opt(1990, 2, 1).unwrap())
            })
        );
        assert_eq!(
            parse_date_range(b"-202002").ok(),
            Some(DateRange {
                start: None,
                end: Some(NaiveDate::from_ymd_opt(2020, 2, 29).unwrap())
            })
        );
        assert_eq!(
            parse_date_range(b"-0020").ok(),
            Some(DateRange {
                start: None,
                end: Some(NaiveDate::from_ymd_opt(20, 12, 31).unwrap())
            })
        );
        assert_eq!(
            parse_date_range(b"0002-").ok(),
            Some(DateRange {
                start: Some(NaiveDate::from_ymd_opt(2, 1, 1).unwrap()),
                end: None
            })
        );
        assert_eq!(
            parse_date_range(b"000203-").ok(),
            Some(DateRange {
                start: Some(NaiveDate::from_ymd_opt(2, 3, 1).unwrap()),
                end: None
            })
        );
        assert_eq!(
            parse_date_range(b"00020307-").ok(),
            Some(DateRange {
                start: Some(NaiveDate::from_ymd_opt(2, 3, 7).unwrap()),
                end: None
            })
        );
        assert_eq!(
            parse_date_range(b"0002-202002  ").ok(),
            Some(DateRange {
                start: Some(NaiveDate::from_ymd_opt(2, 1, 1).unwrap()),
                end: Some(NaiveDate::from_ymd_opt(2020, 2, 29).unwrap())
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
                end: Some(NaiveTime::from_hms_micro_opt(10, 10, 10, 123_456).unwrap())
            })
        );
        assert_eq!(
            parse_time_range(b"-101010.123 ").ok(),
            Some(TimeRange {
                start: None,
                end: Some(NaiveTime::from_hms_micro_opt(10, 10, 10, 123_999).unwrap())
            })
        );
        assert_eq!(
            parse_time_range(b"-01 ").ok(),
            Some(TimeRange {
                start: None,
                end: Some(NaiveTime::from_hms_micro_opt(01, 59, 59, 999_999).unwrap())
            })
        );
        assert_eq!(
            parse_time_range(b"101010.123456-").ok(),
            Some(TimeRange {
                start: Some(NaiveTime::from_hms_micro_opt(10, 10, 10, 123_456).unwrap()),
                end: None
            })
        );
        assert_eq!(
            parse_time_range(b"101010.123-").ok(),
            Some(TimeRange {
                start: Some(NaiveTime::from_hms_micro_opt(10, 10, 10, 123_000).unwrap()),
                end: None
            })
        );
        assert_eq!(
            parse_time_range(b"1010-").ok(),
            Some(TimeRange {
                start: Some(NaiveTime::from_hms_opt(10, 10, 0).unwrap()),
                end: None
            })
        );
        assert_eq!(
            parse_time_range(b"00-").ok(),
            Some(TimeRange {
                start: Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
                end: None
            })
        );
    }

    #[test]
    fn test_parse_datetime_range() {
        
        assert_eq!(
            parse_datetime_range(b"-20200229153420.123456").ok(),
            Some(DateTimeRange::Naive {
                start: None,
                end: Some(
                    NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                            NaiveTime::from_hms_micro_opt(15, 34, 20, 123_456).unwrap()
                        )
                )
            })
        );
        assert_eq!(
            parse_datetime_range(b"-20200229153420.123").ok(),
            Some(DateTimeRange::Naive {
                start: None,
                end: Some(
                    NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                            NaiveTime::from_hms_micro_opt(15, 34, 20, 123_999).unwrap()
                        )
                )
            })
        );
        assert_eq!(
            parse_datetime_range(b"-20200229153420").ok(),
            Some(DateTimeRange::Naive {
                start: None,
                end: Some(
                    NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                            NaiveTime::from_hms_micro_opt(15, 34, 20, 999_999).unwrap()
                        )
                )
            })
        );
        assert_eq!(
            parse_datetime_range(b"-2020022915").ok(),
            Some(DateTimeRange::Naive {
                start: None,
                end: Some(
                    NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                            NaiveTime::from_hms_micro_opt(15, 59, 59, 999_999).unwrap()
                        )
                )
            })
        );
        assert_eq!(
            parse_datetime_range(b"-202002").ok(),
            Some(DateTimeRange::Naive {
                start: None,
                end: Some(
                    NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                            NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
                        )
                )
            })
        );
        assert_eq!(
            parse_datetime_range(b"0002-").ok(),
            Some(DateTimeRange::Naive {
                start: Some(
                    NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(2, 1, 1).unwrap(),
                            NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
                        )
                ),
                end: None
            })
        );
        assert_eq!(
            parse_datetime_range(b"00021231-").ok(),
            Some(DateTimeRange::Naive {
                start: Some(
                    NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(2, 12, 31).unwrap(),
                            NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
                        )
                ),
                end: None
            })
        );
        // two 'east' UTC offsets get parsed
        assert_eq!(
            parse_datetime_range(b"19900101+0500-1999+1400").ok(),
            Some(DateTimeRange::WithTimeZone {
                start: Some(
                    FixedOffset::east_opt(5 * 3600)
                        .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                            NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
                        ))
                        .unwrap()
                ),
                end: Some(
                    FixedOffset::east_opt(14 * 3600)
                        .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(1999, 12, 31).unwrap(),
                            NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
                        ))
                        .unwrap()
                )
            })
        );
        // two 'west' Time zone offsets get parsed
        assert_eq!(
            parse_datetime_range(b"19900101-0500-1999-1200").ok(),
            Some(DateTimeRange::WithTimeZone {
                start: Some(
                    FixedOffset::west_opt(5 * 3600)
                        .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                            NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
                        ))
                        .unwrap()
                ),
                end: Some(
                    FixedOffset::west_opt(12 * 3600)
                        .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(1999, 12, 31).unwrap(),
                            NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
                        ))
                        .unwrap()
                )
            })
        );
        // 'east' and 'west' Time zone offsets get parsed
        assert_eq!(
            parse_datetime_range(b"19900101+1400-1999-1200").ok(),
            Some(DateTimeRange::WithTimeZone {
                start: Some(
                    FixedOffset::east_opt(14 * 3600)
                        .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                            NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
                        ))
                        .unwrap()
                ),
                end: Some(
                    FixedOffset::west_opt(12 * 3600)
                        .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(1999, 12, 31).unwrap(),
                            NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
                        ))
                        .unwrap()
                )
            })
        );
        // one 'west' Time zone offset gets parsed, offset cannot be mistaken for a date-time
        // the missing Time zone offset of the upper bound is generated as a east zero offset.  
        assert_eq!(
            parse_datetime_range(b"19900101-1200-1999").unwrap(),
            DateTimeRange::WithTimeZone  {
                start: Some(
                    FixedOffset::west_opt(12 * 3600)
                        .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                            NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
                        ))
                        .unwrap()
                ),
                end: Some(
                    FixedOffset::east_opt(0)
                    .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(1999, 12, 31).unwrap(),
                            NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
                        ))
                        .unwrap()
                )
            }
        );
        // '0500' can either be a valid west UTC offset on the lower bound, or a valid date-time on the upper bound
        // Now, the first dash is considered to be a range separator, so the lower bound time-zone offset is missing
        // and will be generated to be a east zero offset.
        assert_eq!(
            parse_datetime_range(b"0050-0500-1000").unwrap(),
            DateTimeRange::WithTimeZone {
                start: Some(
                    FixedOffset::east_opt(0)
                    .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(50, 1, 1).unwrap(),
                            NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
                        ))
                        .unwrap()
                ),
                end: Some(
                    FixedOffset::west_opt(10 * 3600)
                        .unwrap()
                        .from_local_datetime(&NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(500, 12, 31).unwrap(),
                            NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
                        ))
                        .unwrap()
                )
            }
        );
        // sequence with more than 3 dashes '-' is refused.
        assert!(matches!(
            parse_datetime_range(b"0001-00021231-2021-0100-0100"),
            Err(Error::SeparatorCount { .. })
        ));
        // any sequence without a dash '-' is refused.
        assert!(matches!(
            parse_datetime_range(b"00021231+0500"),
            Err(Error::NoRangeSeparator { .. })
        ));
    }
}
