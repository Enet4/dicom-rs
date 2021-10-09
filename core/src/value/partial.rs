//! Handling of partial precision of Date, Time and DateTime values.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveTime, TimeZone, Timelike};
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::ops::RangeInclusive;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Date is invalid"))]
    InvalidDate { backtrace: Backtrace },
    #[snafu(display("Time is invalid"))]
    InvalidTime { backtrace: Backtrace },
    #[snafu(display("DateTime is invalid"))]
    InvalidDateTime { backtrace: Backtrace },
    #[snafu(display("To combine a DicomDate with a DicomTime value, the DicomDate has to be precise. Precision is: '{:?}'", value))]
    DateTimeFromPartials {
        value: DateComponent,
        backtrace: Backtrace,
    },
    #[snafu(display(
        "'{:?}' has invalid value: '{}', must be in {:?}",
        component,
        value,
        range
    ))]
    InvalidComponent {
        component: DateComponent,
        value: u32,
        range: RangeInclusive<u32>,
        backtrace: Backtrace,
    },
    #[snafu(display(
        "Second fraction precision '{}' is out of range, must be in 0..=6",
        value
    ))]
    FractionPrecisionRange { value: u32, backtrace: Backtrace },
    #[snafu(display(
        "Number of digits in decimal representation of fraction '{}' does not match it's precision '{}'",
        fraction,
        precision
    ))]
    FractionPrecisionMismatch {
        fraction: u32,
        precision: u32,
        backtrace: Backtrace,
    },
    #[snafu(display("Conversion of value '{}' into {:?} failed", value, component))]
    Conversion {
        value: String,
        component: DateComponent,
        source: std::num::TryFromIntError,
    },
    #[snafu(display(
        "Cannot convert from an imprecise value. This value represents a date / time range"
    ))]
    ImpreciseValue { backtrace: Backtrace },
}

type Result<T, E = Error> = std::result::Result<T, E>;

/// Represents components of Date, Time and DateTime values.
#[derive(Debug, PartialEq, Copy, Clone, Eq, Hash, PartialOrd, Ord)]
pub enum DateComponent {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Millisecond,
    Fraction,
    UtcOffset,
}

/// Represents a Dicom Date value with a partial precision,
/// where some date components may be missing.
///
/// Unlike RUSTs `chrono::NaiveDate`, it does not allow for negative years.
///
/// `DicomDate` implements `AsRange` trait, enabling to retrieve specific
/// `chrono::NaiveDate` values.
///
/// # Example
/// ```
/// # use std::error::Error;
/// # use std::convert::TryFrom;
/// use chrono::NaiveDate;
/// use dicom_core::value::{DicomDate, AsRange};
/// # fn main() -> Result<(), Box<dyn Error>> {
///
/// let date = DicomDate::from_y(1492)?;
///
/// assert_eq!(
///     date.latest()?,
///     NaiveDate::from_ymd(1492,12,31)
/// );
///
/// let date = DicomDate::try_from(&NaiveDate::from_ymd(1900, 5, 3))?;
/// // conversion from chrono value leads to a precise value
/// assert_eq!(date.is_precise(), true);
///
/// assert_eq!(date.to_encoded(), "19000503");
/// # Ok(())
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DicomDate(DicomDateImpl);

/// Represents a Dicom Time value with a partial precision,
/// where some time components may be missing.
///
/// Unlike Ruts's `chrono::NaiveTime`, this implemenation has only 6 digit precision
/// for fraction of a second.
///
/// `DicomTime` implements `AsRange` trait, enabling to retrieve specific
/// `chrono::NaiveTime` values.
///
/// # Example
/// ```
/// # use std::error::Error;
/// # use std::convert::TryFrom;
/// use chrono::NaiveTime;
/// use dicom_core::value::{DicomTime, AsRange};
/// # fn main() -> Result<(), Box<dyn Error>> {
///
/// let time = DicomTime::from_hm(12, 30)?;
///
/// assert_eq!(
///     time.latest()?,
///     NaiveTime::from_hms_micro(12, 30, 59, 999_999)
/// );
///
/// let milli = DicomTime::from_hms_milli(12, 30, 59, 123)?;
///
/// // value still not precise to microsecond
/// assert_eq!(milli.is_precise(), false);
///
/// assert_eq!(milli.to_encoded(), "123059.123");
///
/// // for convenience, is precise enough to be retrieved as a NaiveTime
/// assert_eq!(
///     milli.to_naive_time()?,
///     NaiveTime::from_hms_micro(12, 30, 59, 123_000)
/// );
///
/// let time = DicomTime::try_from(&NaiveTime::from_hms(12, 30, 59))?;
/// // conversion from chrono value leads to a precise value
/// assert_eq!(time.is_precise(), true);
///
/// # Ok(())
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DicomTime(DicomTimeImpl);

/// `DicomDate` is internally represented as this enum.
/// It has 3 possible variants for YYYY, YYYYMM, YYYYMMDD values.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DicomDateImpl {
    Year(u16),
    Month(u16, u8),
    Day(u16, u8, u8),
}

/// `DicomTime` is internally represented as this enum.
/// It has 4 possible variants.
/// The `Fraction` variant stores the fraction second value as `u32`
/// followed by fraction precision as `u8` ranging from 1 to 6.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DicomTimeImpl {
    Hour(u8),
    Minute(u8, u8),
    Second(u8, u8, u8),
    Fraction(u8, u8, u8, u32, u8),
}

/// Represents a Dicom DateTime value with a partial precision,
/// where some date / time components may be missing.
/// `DicomDateTime` is always internally represented by a `DicomDate`
/// and optionally by a `DicomTime`.
/// It implements `AsRange` trait and also holds a `FixedOffset` value, from which corresponding
/// `chrono::DateTime` values can be retrieved.
/// # Example
/// ```
/// # use std::error::Error;
/// # use std::convert::TryFrom;
/// use chrono::{DateTime, FixedOffset, TimeZone};
/// use dicom_core::value::{DicomDate, DicomTime, DicomDateTime, AsRange};
/// # fn main() -> Result<(), Box<dyn Error>> {
///
/// let offset = FixedOffset::east(3600);
///
/// // the least precise date-time value possible is a 'YYYY'
/// let dt = DicomDateTime::from_dicom_date(
///     DicomDate::from_y(2020)?,
///     offset
/// );
/// assert_eq!(
///     dt.earliest()?,
///     offset.ymd(2020, 1, 1)
///     .and_hms(0, 0, 0)
/// );
/// assert_eq!(
///     dt.latest()?,
///     offset.ymd(2020, 12, 31)
///     .and_hms_micro(23, 59, 59, 999_999)
/// );
///
/// let dt = DicomDateTime::try_from(&offset
///     .ymd(2020, 12, 31)
///     .and_hms(23, 59, 0)
///     )?;
/// // conversion from chrono value leads to a precise value
/// assert_eq!(dt.is_precise(), true);
///
/// assert_eq!(dt.to_encoded(), "20201231235900.0+0100");
/// # Ok(())
/// }
/// ```
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct DicomDateTime {
    date: DicomDate,
    time: Option<DicomTime>,
    offset: FixedOffset,
}

/**
 * Throws a detailed `InvalidComponent` error if date / time components are out of range.
 */
pub fn check_component<T>(component: DateComponent, value: &T) -> Result<()>
where
    T: Into<u32> + Copy,
{
    let range = match component {
        DateComponent::Year => 0..=9_999,
        DateComponent::Month => 1..=12,
        DateComponent::Day => 1..=31,
        DateComponent::Hour => 0..=23,
        DateComponent::Minute => 0..=59,
        DateComponent::Second => 0..=60,
        DateComponent::Millisecond => 0..=999,
        DateComponent::Fraction => 0..=999_999,
        DateComponent::UtcOffset => 0..=86_399,
    };

    let value: u32 = (*value).into();
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

impl DicomDate {
    /**
     * Constructs a new `DicomDate` with year precision
     * (YYYY)
     */
    pub fn from_y(year: u16) -> Result<DicomDate> {
        check_component(DateComponent::Year, &year)?;
        Ok(DicomDate(DicomDateImpl::Year(year)))
    }
    /**
     * Constructs a new `DicomDate` with year and month precision
     * (YYYYMM)
     */
    pub fn from_ym(year: u16, month: u8) -> Result<DicomDate> {
        check_component(DateComponent::Year, &year)?;
        check_component(DateComponent::Month, &month)?;
        Ok(DicomDate(DicomDateImpl::Month(year, month)))
    }
    /**
     * Constructs a new `DicomDate` with a year, month and day precision
     * (YYYYMMDD)
     */
    pub fn from_ymd(year: u16, month: u8, day: u8) -> Result<DicomDate> {
        check_component(DateComponent::Year, &year)?;
        check_component(DateComponent::Month, &month)?;
        check_component(DateComponent::Day, &day)?;
        Ok(DicomDate(DicomDateImpl::Day(year, month, day)))
    }
}

impl TryFrom<&NaiveDate> for DicomDate {
    type Error = Error;
    fn try_from(date: &NaiveDate) -> Result<Self> {
        let year: u16 = date.year().try_into().context(Conversion {
            value: date.year().to_string(),
            component: DateComponent::Year,
        })?;
        let month: u8 = date.month().try_into().context(Conversion {
            value: date.month().to_string(),
            component: DateComponent::Month,
        })?;
        let day: u8 = date.day().try_into().context(Conversion {
            value: date.day().to_string(),
            component: DateComponent::Day,
        })?;
        DicomDate::from_ymd(year, month, day)
    }
}

impl fmt::Display for DicomDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DicomDate(DicomDateImpl::Year(y)) => write!(f, "{:04}-MM-DD", y),
            DicomDate(DicomDateImpl::Month(y, m)) => write!(f, "{:04}-{:02}-DD", y, m),
            DicomDate(DicomDateImpl::Day(y, m, d)) => write!(f, "{:04}-{:02}-{:02}", y, m, d),
        }
    }
}

impl DicomTime {
    /**
     * Constructs a new `DicomTime` with hour precision
     * (HH).
     */
    pub fn from_h(hour: u8) -> Result<DicomTime> {
        check_component(DateComponent::Hour, &hour)?;
        Ok(DicomTime(DicomTimeImpl::Hour(hour)))
    }

    /**
     * Constructs a new `DicomTime` with hour and minute precision
     * (HHMM).
     */
    pub fn from_hm(hour: u8, minute: u8) -> Result<DicomTime> {
        check_component(DateComponent::Hour, &hour)?;
        check_component(DateComponent::Minute, &minute)?;
        Ok(DicomTime(DicomTimeImpl::Minute(hour, minute)))
    }

    /**
     * Constructs a new `DicomTime` with hour, minute and second precision
     * (HHMMSS).
     */
    pub fn from_hms(hour: u8, minute: u8, second: u8) -> Result<DicomTime> {
        check_component(DateComponent::Hour, &hour)?;
        check_component(DateComponent::Minute, &minute)?;
        check_component(DateComponent::Second, &second)?;
        Ok(DicomTime(DicomTimeImpl::Second(hour, minute, second)))
    }
    /**
     * Constructs a new `DicomTime` from an hour, minute, second and millisecond value,
     * which leads to a (HHMMSS.FFF) precision. Millisecond cannot exceed `999`.
     */
    pub fn from_hms_milli(hour: u8, minute: u8, second: u8, millisecond: u32) -> Result<DicomTime> {
        check_component(DateComponent::Millisecond, &millisecond)?;
        Ok(DicomTime(DicomTimeImpl::Fraction(
            hour,
            minute,
            second,
            millisecond,
            3,
        )))
    }

    /**
     * Constructs a new `DicomTime` from an hour, minute, second and microsecond value,
     * which leads to full (HHMMSS.FFFFFF) precision. Microsecond cannot exceed `999_999`.
     */
    pub fn from_hms_micro(hour: u8, minute: u8, second: u8, microsecond: u32) -> Result<DicomTime> {
        check_component(DateComponent::Fraction, &microsecond)?;
        Ok(DicomTime(DicomTimeImpl::Fraction(
            hour,
            minute,
            second,
            microsecond,
            6,
        )))
    }

    /**
     * Constructs a new `DicomTime` from an hour, minute, second, second fraction
     * and fraction precision value (1-6). Function used for parsing only.
     */
    pub(crate) fn from_hmsf(
        hour: u8,
        minute: u8,
        second: u8,
        fraction: u32,
        frac_precision: u8,
    ) -> Result<DicomTime> {
        if !(1..=6).contains(&frac_precision) {
            return FractionPrecisionRange {
                value: frac_precision,
            }
            .fail();
        }
        if u32::pow(10, frac_precision as u32) < fraction {
            return FractionPrecisionMismatch {
                fraction,
                precision: frac_precision,
            }
            .fail();
        }

        check_component(DateComponent::Hour, &hour)?;
        check_component(DateComponent::Minute, &minute)?;
        check_component(DateComponent::Second, &second)?;
        let f: u32 = fraction * u32::pow(10, 6 - frac_precision as u32);
        check_component(DateComponent::Fraction, &f)?;
        Ok(DicomTime(DicomTimeImpl::Fraction(
            hour,
            minute,
            second,
            fraction,
            frac_precision,
        )))
    }
}

impl TryFrom<&NaiveTime> for DicomTime {
    type Error = Error;
    fn try_from(time: &NaiveTime) -> Result<Self> {
        let hour: u8 = time.hour().try_into().context(Conversion {
            value: time.hour().to_string(),
            component: DateComponent::Hour,
        })?;
        let minute: u8 = time.minute().try_into().context(Conversion {
            value: time.minute().to_string(),
            component: DateComponent::Minute,
        })?;
        let second: u8 = time.second().try_into().context(Conversion {
            value: time.second().to_string(),
            component: DateComponent::Second,
        })?;
        DicomTime::from_hms_micro(hour, minute, second, time.nanosecond() / 1000)
    }
}

impl fmt::Display for DicomTime {
    fn fmt(&self, frm: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DicomTime(DicomTimeImpl::Hour(h)) => write!(frm, "{:02}:mm:ss.F", h),
            DicomTime(DicomTimeImpl::Minute(h, m)) => write!(frm, "{:02}:{:02}:ss.F", h, m),
            DicomTime(DicomTimeImpl::Second(h, m, s)) => {
                write!(frm, "{:02}:{:02}:{:02}.F", h, m, s)
            }
            DicomTime(DicomTimeImpl::Fraction(h, m, s, f, _fp)) => {
                write!(frm, "{:02}:{:02}:{:02}.{:F<6}", h, m, s, f)
            }
        }
    }
}

impl DicomDateTime {
    /**
     * Constructs a new `DicomDateTime` from a `DicomDate` and a given `FixedOffset`.
     */
    pub fn from_dicom_date(date: DicomDate, offset: FixedOffset) -> DicomDateTime {
        DicomDateTime {
            date,
            time: None,
            offset,
        }
    }

    /**
     * Constructs a new `DicomDateTime` from a `DicomDate`, `DicomTime` and a given `FixedOffset`,
     * providing that `DicomDate` is precise.
     */
    pub fn from_dicom_date_and_time(
        date: DicomDate,
        time: DicomTime,
        offset: FixedOffset,
    ) -> Result<DicomDateTime> {
        if date.is_precise() {
            Ok(DicomDateTime {
                date,
                time: Some(time),
                offset,
            })
        } else {
            DateTimeFromPartials {
                value: date.precision(),
            }
            .fail()
        }
    }
}

impl TryFrom<&DateTime<FixedOffset>> for DicomDateTime {
    type Error = Error;
    fn try_from(dt: &DateTime<FixedOffset>) -> Result<Self> {
        let year: u16 = dt.year().try_into().context(Conversion {
            value: dt.year().to_string(),
            component: DateComponent::Year,
        })?;
        let month: u8 = dt.month().try_into().context(Conversion {
            value: dt.month().to_string(),
            component: DateComponent::Month,
        })?;
        let day: u8 = dt.day().try_into().context(Conversion {
            value: dt.day().to_string(),
            component: DateComponent::Day,
        })?;
        let hour: u8 = dt.hour().try_into().context(Conversion {
            value: dt.hour().to_string(),
            component: DateComponent::Hour,
        })?;
        let minute: u8 = dt.minute().try_into().context(Conversion {
            value: dt.minute().to_string(),
            component: DateComponent::Minute,
        })?;
        let second: u8 = dt.second().try_into().context(Conversion {
            value: dt.second().to_string(),
            component: DateComponent::Second,
        })?;

        DicomDateTime::from_dicom_date_and_time(
            DicomDate::from_ymd(year, month, day)?,
            DicomTime::from_hms_micro(hour, minute, second, dt.nanosecond() / 1000)?,
            *dt.offset(),
        )
    }
}

impl fmt::Display for DicomDateTime {
    fn fmt(&self, frm: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.time {
            None => write!(frm, "{} {}", self.date, self.offset),
            Some(time) => write!(frm, "{} {} {}", self.date, time, self.offset),
        }
    }
}

/**
 * This trait is implemented by partial precision
 * Date, Time and DateTime structures.
 * Trait method returns the last fully precise `DateComponent` of the structure.
 */
pub trait Precision {
    fn precision(&self) -> DateComponent;
}

impl Precision for DicomDate {
    fn precision(&self) -> DateComponent {
        match self {
            DicomDate(DicomDateImpl::Year(..)) => DateComponent::Year,
            DicomDate(DicomDateImpl::Month(..)) => DateComponent::Month,
            DicomDate(DicomDateImpl::Day(..)) => DateComponent::Day,
        }
    }
}

impl Precision for DicomTime {
    fn precision(&self) -> DateComponent {
        match self {
            DicomTime(DicomTimeImpl::Hour(..)) => DateComponent::Hour,
            DicomTime(DicomTimeImpl::Minute(..)) => DateComponent::Minute,
            DicomTime(DicomTimeImpl::Second(..)) => DateComponent::Second,
            DicomTime(DicomTimeImpl::Fraction(..)) => DateComponent::Fraction,
        }
    }
}

impl Precision for DicomDateTime {
    fn precision(&self) -> DateComponent {
        match self.time {
            Some(time) => time.precision(),
            None => self.date.precision(),
        }
    }
}

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
/// use dicom_core::value::{AsRange, DicomDate, DicomTime};
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
///     (Some(NaiveTime::from_hms(10, 0, 0)),
///      Some(NaiveTime::from_hms_micro(10, 0, 59, 999_999)))
/// );
/// // only a time with 6 digits second fraction is considered precise
/// assert!(dicom_time.exact().is_err());
///
/// # Ok(())
/// # }
/// ```
pub trait AsRange: Precision {
    type Item: PartialEq + PartialOrd;
    /**
     * Returns a corresponding `chrono` value, if the partial precision structure has full accuracy.
     */
    fn exact(&self) -> Result<Self::Item> {
        if self.is_precise() {
            Ok(self.earliest()?)
        } else {
            ImpreciseValue.fail()
        }
    }
    /**
     * Returns the earliest possible `chrono` value from a partial precision structure.
     * Missing components default to 1 (days, months) or 0 (hours, minutes, ...)
     * If structure contains invalid combination of `DateComponent`s, it fails.
     */
    fn earliest(&self) -> Result<Self::Item>;

    /**
     * Returns the latest possible `chrono` value from a partial precision structure.
     * If structure contains invalid combination of `DateComponent`s, it fails.
     */
    fn latest(&self) -> Result<Self::Item>;

    /**
     * Returns a tuple of the earliest and latest possible value from a partial precision structure.
     *
     */
    #[allow(clippy::type_complexity)]
    fn range(&self) -> Result<(Option<Self::Item>, Option<Self::Item>)> {
        Ok((self.earliest().ok(), self.latest().ok()))
    }

    /**
     * Returns `true`, if partial precision structure has maximum possible accuracy.
     * For fraction of a second, full 6 digits are required for the value to be precise.
     */
    fn is_precise(&self) -> bool {
        let e = self.earliest();
        let l = self.latest();

        e.is_ok() && l.is_ok() && e.ok() == l.ok()
    }
}

impl AsRange for DicomDate {
    type Item = NaiveDate;
    fn earliest(&self) -> Result<NaiveDate> {
        let (y, m, d) = match self {
            DicomDate(DicomDateImpl::Year(y)) => (*y as i32, 1, 1),
            DicomDate(DicomDateImpl::Month(y, m)) => (*y as i32, *m as u32, 1),
            DicomDate(DicomDateImpl::Day(y, m, d)) => (*y as i32, *m as u32, *d as u32),
        };
        NaiveDate::from_ymd_opt(y, m, d).context(InvalidDate)
    }

    fn latest(&self) -> Result<NaiveDate> {
        let (y, m, d) = match self {
            DicomDate(DicomDateImpl::Year(y)) => (*y as i32, 12, 31),
            DicomDate(DicomDateImpl::Month(y, m)) => {
                let d = {
                    if m == &12 {
                        NaiveDate::from_ymd(*y as i32 + 1, 1, 1)
                    } else {
                        NaiveDate::from_ymd(*y as i32, *m as u32 + 1, 1)
                    }
                    .signed_duration_since(NaiveDate::from_ymd(*y as i32, *m as u32, 1))
                    .num_days()
                };
                (*y as i32, *m as u32, d as u32)
            }
            DicomDate(DicomDateImpl::Day(y, m, d)) => (*y as i32, *m as u32, *d as u32),
        };
        NaiveDate::from_ymd_opt(y, m, d).context(InvalidDate)
    }
}

impl AsRange for DicomTime {
    type Item = NaiveTime;
    fn earliest(&self) -> Result<NaiveTime> {
        let fr: u32;
        let (h, m, s, f) = match self {
            DicomTime(DicomTimeImpl::Hour(h)) => (h, &0, &0, &0),
            DicomTime(DicomTimeImpl::Minute(h, m)) => (h, m, &0, &0),
            DicomTime(DicomTimeImpl::Second(h, m, s)) => (h, m, s, &0),
            DicomTime(DicomTimeImpl::Fraction(h, m, s, f, fp)) => {
                fr = *f * u32::pow(10, 6 - <u32>::from(*fp));
                (h, m, s, &fr)
            }
        };
        NaiveTime::from_hms_micro_opt((*h).into(), (*m).into(), (*s).into(), *f)
            .context(InvalidTime)
    }
    fn latest(&self) -> Result<NaiveTime> {
        let fr: u32;
        let (h, m, s, f) = match self {
            DicomTime(DicomTimeImpl::Hour(h)) => (h, &59, &59, &999_999),
            DicomTime(DicomTimeImpl::Minute(h, m)) => (h, m, &59, &999_999),
            DicomTime(DicomTimeImpl::Second(h, m, s)) => (h, m, s, &999_999),
            DicomTime(DicomTimeImpl::Fraction(h, m, s, f, fp)) => {
                fr = (*f * u32::pow(10, 6 - u32::from(*fp))) + (u32::pow(10, 6 - u32::from(*fp)))
                    - 1;
                (h, m, s, &fr)
            }
        };
        NaiveTime::from_hms_micro_opt((*h).into(), (*m).into(), (*s).into(), *f)
            .context(InvalidTime)
    }
}

impl AsRange for DicomDateTime {
    type Item = DateTime<FixedOffset>;
    fn earliest(&self) -> Result<DateTime<FixedOffset>> {
        let date = self.date.earliest()?;
        let time = match self.time {
            Some(time) => time.earliest()?,
            None => NaiveTime::from_hms(0, 0, 0),
        };

        self.offset
            .from_utc_date(&date)
            .and_time(time)
            .context(InvalidDateTime)
    }

    fn latest(&self) -> Result<DateTime<FixedOffset>> {
        let date = self.date.latest()?;
        let time = match self.time {
            Some(time) => time.latest()?,
            None => NaiveTime::from_hms_micro(23, 59, 59, 999_999),
        };
        self.offset
            .from_utc_date(&date)
            .and_time(time)
            .context(InvalidDateTime)
    }
}

impl DicomDate {
    /**
     * Retrieves a `chrono::NaiveDate` if value is precise.
     */
    pub fn to_naive_date(self) -> Result<NaiveDate> {
        self.exact()
    }
    /**
     * Retrieves a dicom encoded string representation of the value.
     */
    pub fn to_encoded(&self) -> String {
        match self {
            DicomDate(DicomDateImpl::Year(y)) => format!("{:04}", y),
            DicomDate(DicomDateImpl::Month(y, m)) => format!("{:04}{:02}", y, m),
            DicomDate(DicomDateImpl::Day(y, m, d)) => format!("{:04}{:02}{:02}", y, m, d),
        }
    }
}

impl DicomTime {
    /**
     * Retrieves a `chrono::NaiveTime` if value is precise.
     * This method consideres a `DicomTime` value to be precise, if it contains a second component.
     * Missing second fraction defaults to zero.
     */
    pub fn to_naive_time(self) -> Result<NaiveTime> {
        match self.precision() {
            DateComponent::Second | DateComponent::Fraction => self.earliest(),
            _ => ImpreciseValue.fail(),
        }
    }
    /**
     * Retrieves a dicom encoded string representation of the value.
     */
    pub fn to_encoded(&self) -> String {
        match self {
            DicomTime(DicomTimeImpl::Hour(h)) => format!("{:02}", h),
            DicomTime(DicomTimeImpl::Minute(h, m)) => format!("{:02}{:02}", h, m),
            DicomTime(DicomTimeImpl::Second(h, m, s)) => format!("{:02}{:02}{:02}", h, m, s),
            DicomTime(DicomTimeImpl::Fraction(h, m, s, f, _fp)) => {
                format!("{:02}{:02}{:02}.{}", h, m, s, f)
            }
        }
    }
}

impl DicomDateTime {
    /**
     * Retrieves a `chrono::DateTime<FixedOffset>` if value is precise.
     */
    pub fn to_chrono_datetime(self) -> Result<DateTime<FixedOffset>> {
        // tweak here, if full DicomTime precision req. proves impractical
        self.exact()
    }
    /**
     * Retrieves a dicom encoded string representation of the value.
     */
    pub fn to_encoded(&self) -> String {
        match self.time {
            Some(time) => format!(
                "{}{}{}",
                self.date.to_encoded(),
                time.to_encoded(),
                self.offset.to_string().replace(":", "")
            ),
            None => format!(
                "{}{}",
                self.date.to_encoded(),
                self.offset.to_string().replace(":", "")
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dicom_date() {
        assert_eq!(
            DicomDate::from_ymd(1944, 2, 29).unwrap(),
            DicomDate(DicomDateImpl::Day(1944, 2, 29))
        );
        assert_eq!(
            DicomDate::from_ym(1944, 2).unwrap(),
            DicomDate(DicomDateImpl::Month(1944, 2))
        );
        assert_eq!(
            DicomDate::from_y(1944).unwrap(),
            DicomDate(DicomDateImpl::Year(1944))
        );

        assert_eq!(DicomDate::from_ymd(1944, 2, 29).unwrap().is_precise(), true);
        assert_eq!(DicomDate::from_ym(1944, 2).unwrap().is_precise(), false);
        assert_eq!(DicomDate::from_y(1944).unwrap().is_precise(), false);
        assert_eq!(
            DicomDate::from_ymd(1944, 2, 29)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveDate::from_ymd(1944, 2, 29)
        );
        assert_eq!(
            DicomDate::from_ymd(1944, 2, 29).unwrap().latest().unwrap(),
            NaiveDate::from_ymd(1944, 2, 29)
        );

        assert_eq!(
            DicomDate::from_y(1944).unwrap().earliest().unwrap(),
            NaiveDate::from_ymd(1944, 1, 1)
        );
        // detects leap year
        assert_eq!(
            DicomDate::from_ym(1944, 2).unwrap().latest().unwrap(),
            NaiveDate::from_ymd(1944, 2, 29)
        );
        assert_eq!(
            DicomDate::from_ym(1945, 2).unwrap().latest().unwrap(),
            NaiveDate::from_ymd(1945, 2, 28)
        );

        assert_eq!(
            DicomDate::try_from(&NaiveDate::from_ymd(1945, 2, 28)).unwrap(),
            DicomDate(DicomDateImpl::Day(1945, 2, 28))
        );

        assert!(matches!(
            DicomDate::try_from(&NaiveDate::from_ymd(-2000, 2, 28)),
            Err(Error::Conversion { .. })
        ));

        assert!(matches!(
            DicomDate::try_from(&NaiveDate::from_ymd(10_000, 2, 28)),
            Err(Error::InvalidComponent {
                component: DateComponent::Year,
                ..
            })
        ));
    }

    #[test]
    fn test_dicom_time() {
        assert_eq!(
            DicomTime::from_hms_micro(9, 1, 1, 123456).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(9, 1, 1, 123456, 6))
        );
        assert_eq!(
            DicomTime::from_hms_micro(9, 1, 1, 1).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(9, 1, 1, 1, 6))
        );
        assert_eq!(
            DicomTime::from_hms(9, 0, 0).unwrap(),
            DicomTime(DicomTimeImpl::Second(9, 0, 0))
        );
        assert_eq!(
            DicomTime::from_hm(23, 59).unwrap(),
            DicomTime(DicomTimeImpl::Minute(23, 59))
        );
        assert_eq!(
            DicomTime::from_h(1).unwrap(),
            DicomTime(DicomTimeImpl::Hour(1))
        );

        assert_eq!(
            DicomTime::from_hms_milli(9, 1, 1, 123)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 123_000)
        );
        assert_eq!(
            DicomTime::from_hms_milli(9, 1, 1, 123)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 123_999)
        );

        assert_eq!(
            DicomTime::from_hms_milli(9, 1, 1, 2)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 002000)
        );
        assert_eq!(
            DicomTime::from_hms_milli(9, 1, 1, 2)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 002999)
        );

        assert_eq!(
            DicomTime::from_hms_micro(9, 1, 1, 123456)
                .unwrap()
                .is_precise(),
            true
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_milli(16, 31, 28, 123)).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(16, 31, 28, 123_000, 6))
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro(16, 31, 28, 123)).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(16, 31, 28, 000123, 6))
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro(16, 31, 28, 1234)).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(16, 31, 28, 001234, 6))
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro(16, 31, 28, 0)).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(16, 31, 28, 0, 6))
        );

        assert!(matches!(
            DicomTime::from_hmsf(9, 1, 1, 1, 7),
            Err(Error::FractionPrecisionRange { value: 7, .. })
        ));

        assert!(matches!(
            DicomTime::from_hms_milli(9, 1, 1, 1000),
            Err(Error::InvalidComponent {
                component: DateComponent::Millisecond,
                ..
            })
        ));

        assert!(matches!(
            DicomTime::from_hmsf(9, 1, 1, 123456, 3),
            Err(Error::FractionPrecisionMismatch {
                fraction: 123456,
                precision: 3,
                ..
            })
        ));

        assert!(matches!(
            DicomTime::try_from(&NaiveTime::from_hms_micro(16, 31, 28, 1_000_000)),
            Err(Error::InvalidComponent {
                component: DateComponent::Fraction,
                ..
            })
        ));

        assert!(matches!(
            DicomTime::from_hmsf(9, 1, 1, 12345, 5).unwrap().exact(),
            Err(Error::ImpreciseValue { .. })
        ));
    }

    #[test]
    fn test_dicom_datetime() {
        let default_offset = FixedOffset::east(0);
        assert_eq!(
            DicomDateTime::from_dicom_date(
                DicomDate::from_ymd(2020, 2, 29).unwrap(),
                default_offset
            ),
            DicomDateTime {
                date: DicomDate::from_ymd(2020, 2, 29).unwrap(),
                time: None,
                offset: default_offset
            }
        );

        assert_eq!(
            DicomDateTime::from_dicom_date(DicomDate::from_ym(2020, 2).unwrap(), default_offset)
                .earliest()
                .unwrap(),
            FixedOffset::east(0)
                .ymd(2020, 2, 1)
                .and_hms_micro(0, 0, 0, 0)
        );

        assert_eq!(
            DicomDateTime::from_dicom_date(DicomDate::from_ym(2020, 2).unwrap(), default_offset)
                .latest()
                .unwrap(),
            FixedOffset::east(0)
                .ymd(2020, 2, 29)
                .and_hms_micro(23, 59, 59, 999_999)
        );

        assert_eq!(
            DicomDateTime::from_dicom_date_and_time(
                DicomDate::from_ymd(2020, 2, 29).unwrap(),
                DicomTime::from_hmsf(23, 59, 59, 10, 2).unwrap(),
                default_offset
            )
            .unwrap()
            .earliest()
            .unwrap(),
            FixedOffset::east(0)
                .ymd(2020, 2, 29)
                .and_hms_micro(23, 59, 59, 100_000)
        );
        assert_eq!(
            DicomDateTime::from_dicom_date_and_time(
                DicomDate::from_ymd(2020, 2, 29).unwrap(),
                DicomTime::from_hmsf(23, 59, 59, 10, 2).unwrap(),
                default_offset
            )
            .unwrap()
            .latest()
            .unwrap(),
            FixedOffset::east(0)
                .ymd(2020, 2, 29)
                .and_hms_micro(23, 59, 59, 109_999)
        );

        assert_eq!(
            DicomDateTime::try_from(
                &FixedOffset::east(0)
                    .ymd(2020, 2, 29)
                    .and_hms_micro(23, 59, 59, 999_999)
            )
            .unwrap(),
            DicomDateTime {
                date: DicomDate::from_ymd(2020, 2, 29).unwrap(),
                time: Some(DicomTime::from_hms_micro(23, 59, 59, 999_999).unwrap()),
                offset: default_offset
            }
        );

        assert_eq!(
            DicomDateTime::try_from(
                &FixedOffset::east(0)
                    .ymd(2020, 2, 29)
                    .and_hms_micro(23, 59, 59, 0)
            )
            .unwrap(),
            DicomDateTime {
                date: DicomDate::from_ymd(2020, 2, 29).unwrap(),
                time: Some(DicomTime::from_hms_micro(23, 59, 59, 0).unwrap()),
                offset: default_offset
            }
        );

        assert!(matches!(
            DicomDateTime::from_dicom_date(
                DicomDate::from_ymd(2021, 2, 29).unwrap(),
                default_offset
            )
            .earliest(),
            Err(Error::InvalidDate { .. })
        ));

        assert!(matches!(
            DicomDateTime::from_dicom_date_and_time(
                DicomDate::from_ym(2020, 2).unwrap(),
                DicomTime::from_hms_milli(23, 59, 59, 999).unwrap(),
                default_offset
            ),
            Err(Error::DateTimeFromPartials {
                value: DateComponent::Month,
                ..
            })
        ));
        assert!(matches!(
            DicomDateTime::from_dicom_date_and_time(
                DicomDate::from_y(1).unwrap(),
                DicomTime::from_hms_micro(23, 59, 59, 10).unwrap(),
                default_offset
            ),
            Err(Error::DateTimeFromPartials {
                value: DateComponent::Year,
                ..
            })
        ));
        assert!(matches!(
            DicomDateTime::from_dicom_date_and_time(
                DicomDate::from_ymd(2000, 1, 1).unwrap(),
                DicomTime::from_hms_milli(23, 59, 59, 10).unwrap(),
                default_offset
            )
            .unwrap()
            .exact(),
            Err(Error::ImpreciseValue { .. })
        ));
    }
}
