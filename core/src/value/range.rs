//! Handling of date, time, date-time ranges. Needed for range matching.
//! Parsing into ranges happens via partial precision  structures (DicomDate, DicomTime,
//! DicomDatime) so ranges can handle null components in date, time, date-time values.
use chrono::{
    DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone,
};
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};

use crate::value::deserialize::{
    parse_date_partial, parse_datetime_partial, parse_time_partial, Error as DeserializeError,
};
use crate::value::partial::{DicomDate, DicomDateTime, DicomTime};

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
    #[snafu(display("Use 'to_precise_datetime' to retrieve a precise value from a date-time"))]
    ToPreciseDateTime { backtrace: Backtrace },
    #[snafu(display(
        "Parsing a date-time range from '{start}' to '{end}' with only one time-zone '{time_zone} value, second time-zone is missing.'"
    ))]
    AmbiguousDtRange {
        start: NaiveDateTime,
        end: NaiveDateTime,
        time_zone: FixedOffset,
        backtrace: Backtrace,
    },
}
type Result<T, E = Error> = std::result::Result<T, E>;

/// The DICOM protocol accepts date (DA) / time (TM) / date-time (DT) values with null components.
///
/// Imprecise values are to be handled as ranges.
///
/// This trait is implemented by date / time structures with partial precision.
///
/// [AsRange::is_precise()] method will check if the given value has full precision. If so, it can be
/// converted with [AsRange::exact()] to a precise value. If not, [AsRange::range()] will yield a
/// date / time / date-time range.
///
/// Please note that precision does not equal validity. A precise 'YYYYMMDD' [DicomDate] can still
/// fail to produce a valid [chrono::NaiveDate]
///
/// # Examples
///
/// ```
/// # use dicom_core::value::{C, PrimitiveValue};
/// # use smallvec::smallvec;
/// # use std::error::Error;
/// use chrono::{NaiveDate, NaiveTime};
/// use dicom_core::value::{AsRange, DicomDate, DicomTime, DateRange, TimeRange};
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
/// let primitive = PrimitiveValue::from("199402");
///
/// // This is the fastest way to get to a useful date value, but it fails not only for invalid
/// // dates but for imprecise ones as well.
/// assert!(primitive.to_naive_date().is_err());
///
/// // Take intermediate steps:
///
/// // Retrieve a DicomDate.
/// // The parser now checks for basic year and month value ranges here.
/// // But, it would not detect invalid dates like 30th of february etc.
/// let dicom_date : DicomDate = primitive.to_date()?;
///
/// // as we have a valid DicomDate value, let's check if it's precise.
/// if dicom_date.is_precise(){
///         // no components are missing, we can proceed by calling .exact()
///         // which calls the `chrono` library
///         let precise_date: NaiveDate = dicom_date.exact()?;
/// }
/// else{
///         // day / month are missing, no need to call the expensive .exact() method - it will fail
///         // retrieve the earliest possible value directly from DicomDate
///         let earliest: NaiveDate = dicom_date.earliest()?;
///
///         // or convert the date to a date range instead
///         let date_range: DateRange = dicom_date.range()?;
///
///         if let Some(start)  = date_range.start(){
///             // the range has a given lower date bound
///         }
///
/// }
///
/// # Ok(())
/// # }
/// ```
pub trait AsRange {
    type PreciseValue: PartialEq + PartialOrd;
    type Range;

    /// returns true if value has all possible date / time components
    fn is_precise(&self) -> bool;

    /// Returns a corresponding precise value, if the partial precision structure has full accuracy.
    fn exact(&self) -> Result<Self::PreciseValue> {
        if self.is_precise() {
            Ok(self.earliest()?)
        } else {
            ImpreciseValueSnafu.fail()
        }
    }

    /// Returns the earliest possible value from a partial precision structure.
    /// Missing components default to 1 (days, months) or 0 (hours, minutes, ...)
    /// If structure contains invalid combination of `DateComponent`s, it fails.
    fn earliest(&self) -> Result<Self::PreciseValue>;

    /// Returns the latest possible value from a partial precision structure.
    /// If structure contains invalid combination of `DateComponent`s, it fails.
    fn latest(&self) -> Result<Self::PreciseValue>;

    /// Returns a tuple of the earliest and latest possible value from a partial precision structure.
    fn range(&self) -> Result<Self::Range>;
}

impl AsRange for DicomDate {
    type PreciseValue = NaiveDate;
    type Range = DateRange;

    fn is_precise(&self) -> bool {
        self.day().is_some()
    }

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

    fn is_precise(&self) -> bool {
        matches!(self.fraction_and_precision(), Some((_fr_, precision)) if precision == &6)
    }

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

    fn is_precise(&self) -> bool {
        match self.time() {
            Some(dicom_time) => dicom_time.is_precise(),
            None => false,
        }
    }

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
            Some(offset) => Ok(PreciseDateTimeResult::TimeZone(
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
            Some(offset) => Ok(PreciseDateTimeResult::TimeZone(
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

        match (start, end) {
            (PreciseDateTimeResult::Naive(start), PreciseDateTimeResult::Naive(end)) => {
                DateTimeRange::from_start_to_end(start, end)
            }
            (PreciseDateTimeResult::TimeZone(start), PreciseDateTimeResult::TimeZone(end)) => {
                DateTimeRange::from_start_to_end_with_time_zone(start, end)
            }

            _ => unreachable!(),
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
        if self.second().is_some() {
            self.earliest()
        } else {
            ImpreciseValueSnafu.fail()
        }
    }
}

impl DicomDateTime {
    /// Retrieves a [PreciseDateTimeResult] from a date-time value.
    /// If the date-time value is not precise or the conversion leads to ambiguous results,
    /// it fails.
    pub fn to_precise_datetime(&self) -> Result<PreciseDateTimeResult> {
        self.exact()
    }

    #[deprecated(since = "0.7.0", note = "Use `to_precise_date_time()`")]
    pub fn to_chrono_datetime(self) -> Result<DateTime<FixedOffset>> {
        ToPreciseDateTimeSnafu.fail()
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
///
/// # Example
/// ```
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// use chrono::{NaiveDate, NaiveTime, NaiveDateTime, DateTime, FixedOffset, TimeZone};
/// use dicom_core::value::DateTimeRange;
///
/// let offset = FixedOffset::west_opt(3600).unwrap();
///
/// let dtr = DateTimeRange::from_start_to_end_with_time_zone(
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
///  # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum DateTimeRange {
    /// DateTime range without time-zone information
    Naive {
        start: Option<NaiveDateTime>,
        end: Option<NaiveDateTime>,
    },
    /// DateTime range with time-zone information
    TimeZone {
        start: Option<DateTime<FixedOffset>>,
        end: Option<DateTime<FixedOffset>>,
    },
}

/// A precise date-time value, that can either be time-zone aware or time-zone naive.
///
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, PartialOrd)]
pub enum PreciseDateTimeResult {
    Naive(NaiveDateTime),
    TimeZone(DateTime<FixedOffset>),
}

impl PreciseDateTimeResult {
    /// Retrieves a reference to a `chrono::DateTime<FixedOffset>` if the result is time-zone aware.
    pub fn as_datetime_with_time_zone(&self) -> Option<&DateTime<FixedOffset>> {
        match self {
            PreciseDateTimeResult::Naive(..) => None,
            PreciseDateTimeResult::TimeZone(value) => Some(value),
        }
    }

    /// Retrieves a reference to a `chrono::NaiveDateTime` if the result is time-zone naive.
    pub fn as_datetime(&self) -> Option<&NaiveDateTime> {
        match self {
            PreciseDateTimeResult::Naive(value) => Some(value),
            PreciseDateTimeResult::TimeZone(..) => None,
        }
    }

    /// Moves out a `chrono::DateTime<FixedOffset>` if the result is time-zone aware.
    pub fn into_datetime_with_time_zone(self) -> Option<DateTime<FixedOffset>> {
        match self {
            PreciseDateTimeResult::Naive(..) => None,
            PreciseDateTimeResult::TimeZone(value) => Some(value),
        }
    }

    /// Moves out a `chrono::NaiveDateTime` if the result is time-zone naive.
    pub fn into_datetime(self) -> Option<NaiveDateTime> {
        match self {
            PreciseDateTimeResult::Naive(value) => Some(value),
            PreciseDateTimeResult::TimeZone(..) => None,
        }
    }

    /// Returns true if result is time-zone aware.
    pub fn has_time_zone(&self) -> bool {
        match self {
            PreciseDateTimeResult::Naive(..) => false,
            PreciseDateTimeResult::TimeZone(..) => true,
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
    /// Constructs a new time-zone aware `DateTimeRange` from two `chrono::DateTime<FixedOffset>` values
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
            Ok(DateTimeRange::TimeZone {
                start: Some(start),
                end: Some(end),
            })
        }
    }

    /// Constructs a new time-zone naive `DateTimeRange` from two `chrono::NaiveDateTime` values
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

    /// Constructs a new time-zone aware `DateTimeRange` beginning with a `chrono::DateTime<FixedOffset>` value
    /// and no upper limit.
    pub fn from_start_with_time_zone(start: DateTime<FixedOffset>) -> DateTimeRange {
        DateTimeRange::TimeZone {
            start: Some(start),
            end: None,
        }
    }

    /// Constructs a new time-zone naive `DateTimeRange` beginning with a `chrono::NaiveDateTime` value
    /// and no upper limit.
    pub fn from_start(start: NaiveDateTime) -> DateTimeRange {
        DateTimeRange::Naive {
            start: Some(start),
            end: None,
        }
    }

    /// Constructs a new time-zone aware `DateTimeRange` with no lower limit, ending with a `chrono::DateTime<FixedOffset>` value.
    pub fn from_end_with_time_zone(end: DateTime<FixedOffset>) -> DateTimeRange {
        DateTimeRange::TimeZone {
            start: None,
            end: Some(end),
        }
    }

    /// Constructs a new time-zone naive `DateTimeRange` with no lower limit, ending with a `chrono::NaiveDateTime` value.
    pub fn from_end(end: NaiveDateTime) -> DateTimeRange {
        DateTimeRange::Naive {
            start: None,
            end: Some(end),
        }
    }

    /// Returns the lower bound of the range, if present.
    pub fn start(&self) -> Option<PreciseDateTimeResult> {
        match self {
            DateTimeRange::Naive { start, .. } => start.map(PreciseDateTimeResult::Naive),
            DateTimeRange::TimeZone { start, .. } => start.map(PreciseDateTimeResult::TimeZone),
        }
    }

    /// Returns the upper bound of the range, if present.
    pub fn end(&self) -> Option<PreciseDateTimeResult> {
        match self {
            DateTimeRange::Naive { start: _, end } => end.map(PreciseDateTimeResult::Naive),
            DateTimeRange::TimeZone { start: _, end } => end.map(PreciseDateTimeResult::TimeZone),
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

/// The Dicom standard allows for parsing a date-time range in which one DT value provides time-zone
/// information but the other does not.
///
/// Example '19750101-19800101+0200'.
///
/// In such case, the missing time-zone can be interpreted as the local time-zone or the time-zone
/// provided with the upper bound (or something else altogether).
/// This trait is implemented by parsers handling the afformentioned situation.
pub trait AmbiguousDtRangeParser {
    /// Retrieve a [DateTimeRange] if the lower range bound is missing a time-zone
    fn parse_with_ambiguous_start(
        ambiguous_start: NaiveDateTime,
        end: DateTime<FixedOffset>,
    ) -> Result<DateTimeRange>;
    /// Retrieve a [DateTimeRange] if the upper range bound is missing a time-zone
    fn parse_with_ambiguous_end(
        start: DateTime<FixedOffset>,
        ambiguous_end: NaiveDateTime,
    ) -> Result<DateTimeRange>;
}

/// For the missing time-zone use time-zone information of the local system clock.
/// Retrieves a [DateTimeRange::TimeZone].
///
/// Because "A Date Time value without the optional suffix is interpreted to be in the local time zone
///  of the application creating the Data Element, unless explicitly specified by the Timezone Offset From UTC (0008,0201).",
/// this is the default behavior of the parser.
/// https://dicom.nema.org/dicom/2013/output/chtml/part05/sect_6.2.html
#[derive(Debug)]
pub struct ToLocalTimeZone;

/// Use time-zone information from the time-zone aware value.
/// Retrieves a [DateTimeRange::TimeZone].
#[derive(Debug)]
pub struct ToKnownTimeZone;

/// Fail if ambiguous date-time range is parsed
#[derive(Debug)]
pub struct FailOnAmbiguousRange;

/// Discard known (parsed) time-zone information.
/// Retrieves a [DateTimeRange::Naive].
#[derive(Debug)]
pub struct IgnoreTimeZone;

impl AmbiguousDtRangeParser for ToKnownTimeZone {
    fn parse_with_ambiguous_start(
        ambiguous_start: NaiveDateTime,
        end: DateTime<FixedOffset>,
    ) -> Result<DateTimeRange> {
        let start = end
            .offset()
            .from_local_datetime(&ambiguous_start)
            .single()
            .context(InvalidDateTimeSnafu {
                naive: ambiguous_start,
                offset: *end.offset(),
            })?;
        if start > end {
            RangeInversionSnafu {
                start: ambiguous_start.to_string(),
                end: end.to_string(),
            }
            .fail()
        } else {
            Ok(DateTimeRange::TimeZone {
                start: Some(start),
                end: Some(end),
            })
        }
    }
    fn parse_with_ambiguous_end(
        start: DateTime<FixedOffset>,
        ambiguous_end: NaiveDateTime,
    ) -> Result<DateTimeRange> {
        let end = start
            .offset()
            .from_local_datetime(&ambiguous_end)
            .single()
            .context(InvalidDateTimeSnafu {
                naive: ambiguous_end,
                offset: *start.offset(),
            })?;
        if start > end {
            RangeInversionSnafu {
                start: start.to_string(),
                end: ambiguous_end.to_string(),
            }
            .fail()
        } else {
            Ok(DateTimeRange::TimeZone {
                start: Some(start),
                end: Some(end),
            })
        }
    }
}

impl AmbiguousDtRangeParser for FailOnAmbiguousRange {
    fn parse_with_ambiguous_end(
        start: DateTime<FixedOffset>,
        end: NaiveDateTime,
    ) -> Result<DateTimeRange> {
        let time_zone = *start.offset();
        let start = start.naive_local();
        AmbiguousDtRangeSnafu {
            start,
            end,
            time_zone,
        }
        .fail()
    }
    fn parse_with_ambiguous_start(
        start: NaiveDateTime,
        end: DateTime<FixedOffset>,
    ) -> Result<DateTimeRange> {
        let time_zone = *end.offset();
        let end = end.naive_local();
        AmbiguousDtRangeSnafu {
            start,
            end,
            time_zone,
        }
        .fail()
    }
}

impl AmbiguousDtRangeParser for ToLocalTimeZone {
    fn parse_with_ambiguous_start(
        ambiguous_start: NaiveDateTime,
        end: DateTime<FixedOffset>,
    ) -> Result<DateTimeRange> {
        let start = Local::now()
            .offset()
            .from_local_datetime(&ambiguous_start)
            .single()
            .context(InvalidDateTimeSnafu {
                naive: ambiguous_start,
                offset: *end.offset(),
            })?;
        if start > end {
            RangeInversionSnafu {
                start: ambiguous_start.to_string(),
                end: end.to_string(),
            }
            .fail()
        } else {
            Ok(DateTimeRange::TimeZone {
                start: Some(start),
                end: Some(end),
            })
        }
    }
    fn parse_with_ambiguous_end(
        start: DateTime<FixedOffset>,
        ambiguous_end: NaiveDateTime,
    ) -> Result<DateTimeRange> {
        let end = Local::now()
            .offset()
            .from_local_datetime(&ambiguous_end)
            .single()
            .context(InvalidDateTimeSnafu {
                naive: ambiguous_end,
                offset: *start.offset(),
            })?;
        if start > end {
            RangeInversionSnafu {
                start: start.to_string(),
                end: ambiguous_end.to_string(),
            }
            .fail()
        } else {
            Ok(DateTimeRange::TimeZone {
                start: Some(start),
                end: Some(end),
            })
        }
    }
}

impl AmbiguousDtRangeParser for IgnoreTimeZone {
    fn parse_with_ambiguous_start(
        ambiguous_start: NaiveDateTime,
        end: DateTime<FixedOffset>,
    ) -> Result<DateTimeRange> {
        let end = end.naive_local();
        if ambiguous_start > end {
            RangeInversionSnafu {
                start: ambiguous_start.to_string(),
                end: end.to_string(),
            }
            .fail()
        } else {
            Ok(DateTimeRange::Naive {
                start: Some(ambiguous_start),
                end: Some(end),
            })
        }
    }
    fn parse_with_ambiguous_end(
        start: DateTime<FixedOffset>,
        ambiguous_end: NaiveDateTime,
    ) -> Result<DateTimeRange> {
        let start = start.naive_local();
        if start > ambiguous_end {
            RangeInversionSnafu {
                start: start.to_string(),
                end: ambiguous_end.to_string(),
            }
            .fail()
        } else {
            Ok(DateTimeRange::Naive {
                start: Some(start),
                end: Some(ambiguous_end),
            })
        }
    }
}

/// Looks for a range separator '-'.
/// Returns a `DateTimeRange`.
///
/// If the parser encounters two date-time values, where one is time-zone aware and the other is not,
/// it will use the local time-zone offset and use it instead of the missing time-zone.
///
/// Because "A Date Time value without the optional suffix is interpreted to be in the local time zone
///  of the application creating the Data Element, unless explicitly specified by the Timezone Offset From UTC (0008,0201).",
/// this is the default behavior of the parser.
/// https://dicom.nema.org/dicom/2013/output/chtml/part05/sect_6.2.html
///
/// To customize this behavior, please use [parse_datetime_range_custom()].
///
/// Users are advised, that for very specific inputs, inconsistent behavior can occur.
/// This behavior can only be produced when all of the following is true:
/// - two very short date-times in the form of YYYY are presented (YYYY-YYYY)
/// - both YYYY values can be exchanged for a valid west UTC offset, meaning year <= 1200 e.g. (1000-1100)
/// - only one west UTC offset is presented. e.g. (1000-1100-0100)
/// In such cases, two '-' characters are present and the parser will favor the first one as a range separator,
/// if it produces a valid `DateTimeRange`. Otherwise, it tries the second one.
pub fn parse_datetime_range(buf: &[u8]) -> Result<DateTimeRange> {
    parse_datetime_range_impl::<ToLocalTimeZone>(buf)
}

/// Same as [parse_datetime_range()] but allows for custom handling of ambiguous Date-time ranges.
/// See [AmbiguousDtRangeParser].
pub fn parse_datetime_range_custom<T: AmbiguousDtRangeParser>(buf: &[u8]) -> Result<DateTimeRange> {
    parse_datetime_range_impl::<T>(buf)
}

pub fn parse_datetime_range_impl<T: AmbiguousDtRangeParser>(buf: &[u8]) -> Result<DateTimeRange> {
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
            PreciseDateTimeResult::TimeZone(end_tz) => {
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
            PreciseDateTimeResult::TimeZone(start_tz) => {
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
                                PreciseDateTimeResult::TimeZone(start),
                                PreciseDateTimeResult::TimeZone(end),
                            ) => DateTimeRange::from_start_to_end_with_time_zone(start, end),
                            (
                                // lower bound time-zone was missing
                                PreciseDateTimeResult::Naive(start),
                                PreciseDateTimeResult::TimeZone(end),
                            ) => T::parse_with_ambiguous_start(start, end),
                            (
                                PreciseDateTimeResult::TimeZone(start),
                                // upper bound time-zone was missing
                                PreciseDateTimeResult::Naive(end),
                            ) => T::parse_with_ambiguous_end(start, end),
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
            (PreciseDateTimeResult::TimeZone(start), PreciseDateTimeResult::TimeZone(end)) => {
                DateTimeRange::from_start_to_end_with_time_zone(start, end)
            }
            // lower bound time-zone was missing
            (PreciseDateTimeResult::Naive(start), PreciseDateTimeResult::TimeZone(end)) => {
                T::parse_with_ambiguous_start(start, end)
            }
            // upper bound time-zone was missing
            (PreciseDateTimeResult::TimeZone(start), PreciseDateTimeResult::Naive(end)) => {
                T::parse_with_ambiguous_end(start, end)
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
            Some(PreciseDateTimeResult::TimeZone(
                offset
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                        NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
                    ))
                    .unwrap()
            ))
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
            Some(PreciseDateTimeResult::TimeZone(
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
            .start(),
            Some(PreciseDateTimeResult::TimeZone(
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
            Some(PreciseDateTimeResult::TimeZone(
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
            DateTimeRange::from_start(NaiveDateTime::new(
                NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
            ))
            .start(),
            Some(PreciseDateTimeResult::Naive(NaiveDateTime::new(
                NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
            )))
        );
        assert_eq!(
            DateTimeRange::from_end(NaiveDateTime::new(
                NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
            ))
            .end(),
            Some(PreciseDateTimeResult::Naive(NaiveDateTime::new(
                NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
            )))
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
            Some(PreciseDateTimeResult::Naive(NaiveDateTime::new(
                NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                NaiveTime::from_hms_micro_opt(1, 1, 1, 1).unwrap()
            )))
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
            Some(PreciseDateTimeResult::Naive(NaiveDateTime::new(
                NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                NaiveTime::from_hms_micro_opt(1, 1, 1, 5).unwrap()
            )))
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
                end: Some(NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                    NaiveTime::from_hms_micro_opt(15, 34, 20, 123_456).unwrap()
                ))
            })
        );
        assert_eq!(
            parse_datetime_range(b"-20200229153420.123").ok(),
            Some(DateTimeRange::Naive {
                start: None,
                end: Some(NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                    NaiveTime::from_hms_micro_opt(15, 34, 20, 123_999).unwrap()
                ))
            })
        );
        assert_eq!(
            parse_datetime_range(b"-20200229153420").ok(),
            Some(DateTimeRange::Naive {
                start: None,
                end: Some(NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                    NaiveTime::from_hms_micro_opt(15, 34, 20, 999_999).unwrap()
                ))
            })
        );
        assert_eq!(
            parse_datetime_range(b"-2020022915").ok(),
            Some(DateTimeRange::Naive {
                start: None,
                end: Some(NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                    NaiveTime::from_hms_micro_opt(15, 59, 59, 999_999).unwrap()
                ))
            })
        );
        assert_eq!(
            parse_datetime_range(b"-202002").ok(),
            Some(DateTimeRange::Naive {
                start: None,
                end: Some(NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                    NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
                ))
            })
        );
        assert_eq!(
            parse_datetime_range(b"0002-").ok(),
            Some(DateTimeRange::Naive {
                start: Some(NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(2, 1, 1).unwrap(),
                    NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
                )),
                end: None
            })
        );
        assert_eq!(
            parse_datetime_range(b"00021231-").ok(),
            Some(DateTimeRange::Naive {
                start: Some(NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(2, 12, 31).unwrap(),
                    NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
                )),
                end: None
            })
        );
        // two 'east' UTC offsets get parsed
        assert_eq!(
            parse_datetime_range(b"19900101+0500-1999+1400").ok(),
            Some(DateTimeRange::TimeZone {
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
            Some(DateTimeRange::TimeZone {
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
            Some(DateTimeRange::TimeZone {
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
        // the missing Time zone offset will be replaced with local clock time-zone offset (default behavior)
        assert_eq!(
            parse_datetime_range(b"19900101-1200-1999").unwrap(),
            DateTimeRange::TimeZone {
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
                    Local::now()
                        .offset()
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
        // and will be considered to be the local clock time-zone offset.
        assert_eq!(
            parse_datetime_range(b"0050-0500-1000").unwrap(),
            DateTimeRange::TimeZone {
                start: Some(
                    Local::now()
                        .offset()
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
