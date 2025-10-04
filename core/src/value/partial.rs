//! Handling of partial precision of Date, Time and DateTime values.

use crate::value::AsRange;
use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use snafu::{Backtrace, ResultExt, Snafu};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::ops::RangeInclusive;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
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
    // year precision
    Year,
    // month precision
    Month,
    // day precision
    Day,
    // hour precision
    Hour,
    // minute precision
    Minute,
    // second precision
    Second,
    // millisecond precision
    Millisecond,
    // microsecond (full second fraction)
    Fraction,
    // West UTC time-zone offset
    UtcWest,
    // East UTC time-zone offset
    UtcEast,
}

/// Represents a Dicom date (DA) value with a partial precision,
/// where some date components may be missing.
///
/// Unlike [chrono::NaiveDate], it does not allow for negative years.
///
/// `DicomDate` implements [AsRange] trait, enabling to retrieve specific
/// [date](NaiveDate) values.
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
///     Some(date.latest()?),
///     NaiveDate::from_ymd_opt(1492,12,31)
/// );
///
/// let date = DicomDate::try_from(&NaiveDate::from_ymd_opt(1900, 5, 3).unwrap())?;
/// // conversion from chrono value leads to a precise value
/// assert_eq!(date.is_precise(), true);
///
/// assert_eq!(date.to_string(), "1900-05-03");
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Copy, PartialEq)]
pub struct DicomDate(DicomDateImpl);

/// Represents a Dicom time (TM) value with a partial precision,
/// where some time components may be missing.
///
/// Unlike [chrono::NaiveTime], this implementation has only 6 digit precision
/// for fraction of a second.
///
/// `DicomTime` implements [AsRange] trait, enabling to retrieve specific
/// [time](NaiveTime) values.
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
///     Some(time.latest()?),
///     NaiveTime::from_hms_micro_opt(12, 30, 59, 999_999)
/// );
///
/// let milli = DicomTime::from_hms_milli(12, 30, 59, 123)?;
///
/// // value still not precise to microsecond
/// assert_eq!(milli.is_precise(), false);
///
/// assert_eq!(milli.to_string(), "12:30:59.123");
///
/// // for convenience, is precise enough to be retrieved as a NaiveTime
/// assert_eq!(
///     Some(milli.to_naive_time()?),
///     NaiveTime::from_hms_micro_opt(12, 30, 59, 123_000)
/// );
///
/// let time = DicomTime::try_from(&NaiveTime::from_hms_opt(12, 30, 59).unwrap())?;
/// // conversion from chrono value leads to a precise value
/// assert_eq!(time.is_precise(), true);
///
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Copy, PartialEq)]
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

/// Represents a Dicom date-time (DT) value with a partial precision,
/// where some date or time components may be missing.
///
/// `DicomDateTime` is always internally represented by a [DicomDate].
/// The [DicomTime] and a timezone [FixedOffset] values are optional.
///
/// It implements [AsRange] trait,
/// which serves to retrieve a [`PreciseDateTime`]
/// from values with missing components.
/// # Example
/// ```
/// # use std::error::Error;
/// # use std::convert::TryFrom;
/// use chrono::{DateTime, FixedOffset, TimeZone, NaiveDateTime, NaiveDate, NaiveTime};
/// use dicom_core::value::{DicomDate, DicomTime, DicomDateTime, AsRange, PreciseDateTime};
/// # fn main() -> Result<(), Box<dyn Error>> {
///
/// let offset = FixedOffset::east_opt(3600).unwrap();
///
/// // lets create the least precise date-time value possible 'YYYY' and make it time-zone aware
/// let dt = DicomDateTime::from_date_with_time_zone(
///     DicomDate::from_y(2020)?,
///     offset
/// );
/// // the earliest possible value is output as a [PreciseDateTime]
/// assert_eq!(
///     dt.earliest()?,
///     PreciseDateTime::TimeZone(
///     offset.from_local_datetime(&NaiveDateTime::new(
///         NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
///         NaiveTime::from_hms_opt(0, 0, 0).unwrap()
///     )).single().unwrap())
/// );
/// assert_eq!(
///     dt.latest()?,
///     PreciseDateTime::TimeZone(
///     offset.from_local_datetime(&NaiveDateTime::new(
///         NaiveDate::from_ymd_opt(2020, 12, 31).unwrap(),
///         NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
///     )).single().unwrap())
/// );
///
/// let chrono_datetime = offset.from_local_datetime(&NaiveDateTime::new(
///         NaiveDate::from_ymd_opt(2020, 12, 31).unwrap(),
///         NaiveTime::from_hms_opt(23, 59, 0).unwrap()
///     )).unwrap();
///
/// let dt = DicomDateTime::try_from(&chrono_datetime)?;
/// // conversion from chrono value leads to a precise value
/// assert_eq!(dt.is_precise(), true);
///
/// assert_eq!(dt.to_string(), "2020-12-31 23:59:00.0 +01:00");
/// # Ok(())
/// # }
/// ```
#[derive(PartialEq, Clone, Copy)]
pub struct DicomDateTime {
    date: DicomDate,
    time: Option<DicomTime>,
    time_zone: Option<FixedOffset>,
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
        DateComponent::UtcWest => 0..=(12 * 3600),
        DateComponent::UtcEast => 0..=(14 * 3600),
    };

    let value: u32 = (*value).into();
    if range.contains(&value) {
        Ok(())
    } else {
        InvalidComponentSnafu {
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
     * (`YYYY`)
     */
    pub fn from_y(year: u16) -> Result<DicomDate> {
        check_component(DateComponent::Year, &year)?;
        Ok(DicomDate(DicomDateImpl::Year(year)))
    }
    /**
     * Constructs a new `DicomDate` with year and month precision
     * (`YYYYMM`)
     */
    pub fn from_ym(year: u16, month: u8) -> Result<DicomDate> {
        check_component(DateComponent::Year, &year)?;
        check_component(DateComponent::Month, &month)?;
        Ok(DicomDate(DicomDateImpl::Month(year, month)))
    }
    /**
     * Constructs a new `DicomDate` with a year, month and day precision
     * (`YYYYMMDD`)
     */
    pub fn from_ymd(year: u16, month: u8, day: u8) -> Result<DicomDate> {
        check_component(DateComponent::Year, &year)?;
        check_component(DateComponent::Month, &month)?;
        check_component(DateComponent::Day, &day)?;
        Ok(DicomDate(DicomDateImpl::Day(year, month, day)))
    }

    /// Retrieves the year from a date as a reference
    pub fn year(&self) -> &u16 {
        match self {
            DicomDate(DicomDateImpl::Year(y)) => y,
            DicomDate(DicomDateImpl::Month(y, _)) => y,
            DicomDate(DicomDateImpl::Day(y, _, _)) => y,
        }
    }
    /// Retrieves the month from a date as a reference
    pub fn month(&self) -> Option<&u8> {
        match self {
            DicomDate(DicomDateImpl::Year(_)) => None,
            DicomDate(DicomDateImpl::Month(_, m)) => Some(m),
            DicomDate(DicomDateImpl::Day(_, m, _)) => Some(m),
        }
    }
    /// Retrieves the day from a date as a reference
    pub fn day(&self) -> Option<&u8> {
        match self {
            DicomDate(DicomDateImpl::Year(_)) => None,
            DicomDate(DicomDateImpl::Month(_, _)) => None,
            DicomDate(DicomDateImpl::Day(_, _, d)) => Some(d),
        }
    }

    /** Retrieves the last fully precise `DateComponent` of the value */
    pub(crate) fn precision(&self) -> DateComponent {
        match self {
            DicomDate(DicomDateImpl::Year(..)) => DateComponent::Year,
            DicomDate(DicomDateImpl::Month(..)) => DateComponent::Month,
            DicomDate(DicomDateImpl::Day(..)) => DateComponent::Day,
        }
    }
}

impl std::str::FromStr for DicomDate {
    type Err = crate::value::DeserializeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (date, _) = crate::value::deserialize::parse_date_partial(s.as_bytes())?;
        Ok(date)
    }
}

impl TryFrom<&NaiveDate> for DicomDate {
    type Error = Error;
    fn try_from(date: &NaiveDate) -> Result<Self> {
        let year: u16 = date.year().try_into().with_context(|_| ConversionSnafu {
            value: date.year().to_string(),
            component: DateComponent::Year,
        })?;
        let month: u8 = date.month().try_into().with_context(|_| ConversionSnafu {
            value: date.month().to_string(),
            component: DateComponent::Month,
        })?;
        let day: u8 = date.day().try_into().with_context(|_| ConversionSnafu {
            value: date.day().to_string(),
            component: DateComponent::Day,
        })?;
        DicomDate::from_ymd(year, month, day)
    }
}

impl fmt::Display for DicomDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DicomDate(DicomDateImpl::Year(y)) => write!(f, "{y:04}"),
            DicomDate(DicomDateImpl::Month(y, m)) => write!(f, "{y:04}-{m:02}"),
            DicomDate(DicomDateImpl::Day(y, m, d)) => write!(f, "{y:04}-{m:02}-{d:02}"),
        }
    }
}

impl fmt::Debug for DicomDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DicomDate(DicomDateImpl::Year(y)) => write!(f, "{y:04}-MM-DD"),
            DicomDate(DicomDateImpl::Month(y, m)) => write!(f, "{y:04}-{m:02}-DD"),
            DicomDate(DicomDateImpl::Day(y, m, d)) => write!(f, "{y:04}-{m:02}-{d:02}"),
        }
    }
}

impl DicomTime {
    /**
     * Constructs a new `DicomTime` with hour precision
     * (`HH`).
     */
    pub fn from_h(hour: u8) -> Result<DicomTime> {
        check_component(DateComponent::Hour, &hour)?;
        Ok(DicomTime(DicomTimeImpl::Hour(hour)))
    }

    /**
     * Constructs a new `DicomTime` with hour and minute precision
     * (`HHMM`).
     */
    pub fn from_hm(hour: u8, minute: u8) -> Result<DicomTime> {
        check_component(DateComponent::Hour, &hour)?;
        check_component(DateComponent::Minute, &minute)?;
        Ok(DicomTime(DicomTimeImpl::Minute(hour, minute)))
    }

    /**
     * Constructs a new `DicomTime` with hour, minute and second precision
     * (`HHMMSS`).
     */
    pub fn from_hms(hour: u8, minute: u8, second: u8) -> Result<DicomTime> {
        check_component(DateComponent::Hour, &hour)?;
        check_component(DateComponent::Minute, &minute)?;
        check_component(DateComponent::Second, &second)?;
        Ok(DicomTime(DicomTimeImpl::Second(hour, minute, second)))
    }
    /**
     * Constructs a new `DicomTime` from an hour, minute, second and millisecond value,
     * which leads to the precision `HHMMSS.FFF`. Millisecond cannot exceed `999`.
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

    /// Constructs a new `DicomTime` from an hour, minute, second and microsecond value,
    /// which leads to the full precision `HHMMSS.FFFFFF`.
    ///
    /// Microsecond cannot exceed `999_999`.
    /// Instead, leap seconds can be represented by setting `second` to 60.
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
    /** Retrieves the hour from a time as a reference */
    pub fn hour(&self) -> &u8 {
        match self {
            DicomTime(DicomTimeImpl::Hour(h)) => h,
            DicomTime(DicomTimeImpl::Minute(h, _)) => h,
            DicomTime(DicomTimeImpl::Second(h, _, _)) => h,
            DicomTime(DicomTimeImpl::Fraction(h, _, _, _, _)) => h,
        }
    }
    /** Retrieves the minute from a time as a reference */
    pub fn minute(&self) -> Option<&u8> {
        match self {
            DicomTime(DicomTimeImpl::Hour(_)) => None,
            DicomTime(DicomTimeImpl::Minute(_, m)) => Some(m),
            DicomTime(DicomTimeImpl::Second(_, m, _)) => Some(m),
            DicomTime(DicomTimeImpl::Fraction(_, m, _, _, _)) => Some(m),
        }
    }
    /** Retrieves the minute from a time as a reference */
    pub fn second(&self) -> Option<&u8> {
        match self {
            DicomTime(DicomTimeImpl::Hour(_)) => None,
            DicomTime(DicomTimeImpl::Minute(_, _)) => None,
            DicomTime(DicomTimeImpl::Second(_, _, s)) => Some(s),
            DicomTime(DicomTimeImpl::Fraction(_, _, s, _, _)) => Some(s),
        }
    }

    /// Retrieves the fraction of a second in milliseconds.
    ///
    /// Only returns `Some(_)` if the time is precise to the millisecond or more.
    /// Any precision beyond the millisecond is discarded.
    pub fn millisecond(&self) -> Option<u32> {
        self.fraction_and_precision().and_then(|(f, fp)| match fp {
            0..=2 => None,
            3 => Some(f),
            4 => Some(f / 10),
            5 => Some(f / 100),
            6 => Some(f / 1_000),
            _ => unreachable!("fp outside expected range 0..=6"),
        })
    }

    /// Retrieves the total known fraction of a second in microseconds.
    ///
    /// Only returns `None` if the time value defines no fraction of a second.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::DicomTime;
    /// let time: DicomTime = "202346.2500".parse()?;
    /// assert_eq!(time.fraction_micro(), Some(250_000));
    ///
    /// let time: DicomTime = "202346".parse()?;
    /// assert_eq!(time.fraction_micro(), None);
    /// # Ok::<(), dicom_core::value::DeserializeError>(())
    /// ```
    pub fn fraction_micro(&self) -> Option<u32> {
        match self.fraction_and_precision() {
            None => None,
            Some((f, 1)) => Some(f * 100_000),
            Some((f, 2)) => Some(f * 10_000),
            Some((f, 3)) => Some(f * 1_000),
            Some((f, 4)) => Some(f * 100),
            Some((f, 5)) => Some(f * 10),
            Some((f, 6)) => Some(f),
            Some((_, _)) => unreachable!("fp outside expected range 0..=6"),
        }
    }

    /// Retrieves the total known fraction of a second in milliseconds.
    ///
    /// This may result in precision loss if
    /// the time value was more precise than 3 decimal places.
    ///
    /// Only returns `None` if the time value defines no fraction of a second.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::DicomTime;
    /// let time: DicomTime = "202346.2500".parse()?;
    /// assert_eq!(time.fraction_ms(), Some(250));
    ///
    /// let time: DicomTime = "202346".parse()?;
    /// assert_eq!(time.fraction_ms(), None);
    /// # Ok::<(), dicom_core::value::DeserializeError>(())
    /// ```
    pub fn fraction_ms(&self) -> Option<u32> {
        match self.fraction_and_precision() {
            None => None,
            Some((f, 1)) => Some(f * 100),
            Some((f, 2)) => Some(f * 10),
            Some((f, 3)) => Some(f),
            Some((f, 4)) => Some(f / 10),
            Some((f, 5)) => Some(f / 100),
            Some((f, 6)) => Some(f / 1_000),
            Some((_, _)) => unreachable!("fp outside expected range 0..=6"),
        }
    }

    /// Retrieves the precision of the fraction of a second,
    /// in number of decimal places (in `0..=6`).
    pub fn fraction_precision(&self) -> u8 {
        match self.fraction_and_precision() {
            None => 0,
            Some((_, fp)) => fp,
        }
    }

    /// Retrieves the fraction of a second and its precision.
    ///
    /// Returns a pair containing
    /// the duration and the precision of that duration.
    pub(crate) fn fraction_and_precision(&self) -> Option<(u32, u8)> {
        match self {
            DicomTime(DicomTimeImpl::Hour(_)) => None,
            DicomTime(DicomTimeImpl::Minute(_, _)) => None,
            DicomTime(DicomTimeImpl::Second(_, _, _)) => None,
            DicomTime(DicomTimeImpl::Fraction(_, _, _, f, fp)) => Some((*f, *fp)),
        }
    }

    /// Retrieves the fraction of a second encoded as a small string.
    /// The length of the string matches the number of known decimal places
    /// in the fraction of a second.
    /// Returns an empty string if the time value defines no fraction of a second.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::DicomTime;
    /// let time: DicomTime = "202346.2500".parse()?;
    /// assert_eq!(&time.fraction_str(), "2500");
    ///
    /// let time: DicomTime = "202346".parse()?;
    /// assert_eq!(&time.fraction_str(), "");
    /// # Ok::<(), dicom_core::value::DeserializeError>(())
    /// ```
    pub fn fraction_str(&self) -> String {
        match self.fraction_and_precision() {
            None | Some((_, 0)) => String::new(),
            Some((f, 1)) => format!("{:01}", f),
            Some((f, 2)) => format!("{:02}", f),
            Some((f, 3)) => format!("{:03}", f),
            Some((f, 4)) => format!("{:04}", f),
            Some((f, 5)) => format!("{:05}", f),
            Some((f, 6)) => format!("{:06}", f),
            Some((_, _)) => unreachable!("fp outside expected range 0..=6"),
        }
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
            return FractionPrecisionRangeSnafu {
                value: frac_precision,
            }
            .fail();
        }
        if u32::pow(10, frac_precision as u32) < fraction {
            return FractionPrecisionMismatchSnafu {
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

    /** Retrieves the last fully precise `DateComponent` of the value */
    pub(crate) fn precision(&self) -> DateComponent {
        match self {
            DicomTime(DicomTimeImpl::Hour(..)) => DateComponent::Hour,
            DicomTime(DicomTimeImpl::Minute(..)) => DateComponent::Minute,
            DicomTime(DicomTimeImpl::Second(..)) => DateComponent::Second,
            DicomTime(DicomTimeImpl::Fraction(..)) => DateComponent::Fraction,
        }
    }
}

impl TryFrom<&NaiveTime> for DicomTime {
    type Error = Error;
    fn try_from(time: &NaiveTime) -> Result<Self> {
        let hour: u8 = time.hour().try_into().with_context(|_| ConversionSnafu {
            value: time.hour().to_string(),
            component: DateComponent::Hour,
        })?;
        let minute: u8 = time.minute().try_into().with_context(|_| ConversionSnafu {
            value: time.minute().to_string(),
            component: DateComponent::Minute,
        })?;
        let second: u8 = time.second().try_into().with_context(|_| ConversionSnafu {
            value: time.second().to_string(),
            component: DateComponent::Second,
        })?;
        let microsecond = time.nanosecond() / 1000;
        // leap second correction: convert (59, 1_000_000 + x) to (60, x)
        let (second, microsecond) = if microsecond >= 1_000_000 && second == 59 {
            (60, microsecond - 1_000_000)
        } else {
            (second, microsecond)
        };

        DicomTime::from_hms_micro(hour, minute, second, microsecond)
    }
}

impl std::str::FromStr for DicomTime {
    type Err = crate::value::DeserializeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (time, _) = crate::value::deserialize::parse_time_partial(s.as_bytes())?;
        Ok(time)
    }
}

impl fmt::Display for DicomTime {
    fn fmt(&self, frm: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DicomTime(DicomTimeImpl::Hour(h)) => write!(frm, "{h:02}"),
            DicomTime(DicomTimeImpl::Minute(h, m)) => write!(frm, "{h:02}:{m:02}"),
            DicomTime(DicomTimeImpl::Second(h, m, s)) => {
                write!(frm, "{h:02}:{m:02}:{s:02}")
            }
            DicomTime(DicomTimeImpl::Fraction(h, m, s, f, fp)) => {
                let sfrac = (u32::pow(10, *fp as u32) + f).to_string();
                write!(
                    frm,
                    "{h:02}:{m:02}:{s:02}.{}",
                    match f {
                        0 => "0",
                        _ => sfrac.get(1..).unwrap(),
                    }
                )
            }
        }
    }
}

impl fmt::Debug for DicomTime {
    fn fmt(&self, frm: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DicomTime(DicomTimeImpl::Hour(h)) => write!(frm, "{h:02}:mm:ss.FFFFFF"),
            DicomTime(DicomTimeImpl::Minute(h, m)) => write!(frm, "{h:02}:{m:02}:ss.FFFFFF"),
            DicomTime(DicomTimeImpl::Second(h, m, s)) => {
                write!(frm, "{h:02}:{m:02}:{s:02}.FFFFFF")
            }
            DicomTime(DicomTimeImpl::Fraction(h, m, s, f, _fp)) => {
                write!(frm, "{h:02}:{m:02}:{s:02}.{f:F<6}")
            }
        }
    }
}

impl DicomDateTime {
    /**
     * Constructs a new `DicomDateTime` from a `DicomDate` and a timezone `FixedOffset`.
     */
    pub fn from_date_with_time_zone(date: DicomDate, time_zone: FixedOffset) -> DicomDateTime {
        DicomDateTime {
            date,
            time: None,
            time_zone: Some(time_zone),
        }
    }

    /**
     * Constructs a new `DicomDateTime` from a `DicomDate` .
     */
    pub fn from_date(date: DicomDate) -> DicomDateTime {
        DicomDateTime {
            date,
            time: None,
            time_zone: None,
        }
    }

    /**
     * Constructs a new `DicomDateTime` from a `DicomDate` and a `DicomTime`,
     * providing that `DicomDate` is precise.
     */
    pub fn from_date_and_time(date: DicomDate, time: DicomTime) -> Result<DicomDateTime> {
        if date.is_precise() {
            Ok(DicomDateTime {
                date,
                time: Some(time),
                time_zone: None,
            })
        } else {
            DateTimeFromPartialsSnafu {
                value: date.precision(),
            }
            .fail()
        }
    }

    /**
     * Constructs a new `DicomDateTime` from a `DicomDate`, `DicomTime` and a timezone `FixedOffset`,
     * providing that `DicomDate` is precise.
     */
    pub fn from_date_and_time_with_time_zone(
        date: DicomDate,
        time: DicomTime,
        time_zone: FixedOffset,
    ) -> Result<DicomDateTime> {
        if date.is_precise() {
            Ok(DicomDateTime {
                date,
                time: Some(time),
                time_zone: Some(time_zone),
            })
        } else {
            DateTimeFromPartialsSnafu {
                value: date.precision(),
            }
            .fail()
        }
    }

    /** Retrieves a reference to the internal date value */
    pub fn date(&self) -> &DicomDate {
        &self.date
    }

    /** Retrieves a reference to the internal time value, if present */
    pub fn time(&self) -> Option<&DicomTime> {
        self.time.as_ref()
    }

    /** Retrieves a reference to the internal time-zone value, if present */
    pub fn time_zone(&self) -> Option<&FixedOffset> {
        self.time_zone.as_ref()
    }

    /** Returns true, if the `DicomDateTime` contains a time-zone */
    pub fn has_time_zone(&self) -> bool {
        self.time_zone.is_some()
    }
}

impl TryFrom<&DateTime<FixedOffset>> for DicomDateTime {
    type Error = Error;
    fn try_from(dt: &DateTime<FixedOffset>) -> Result<Self> {
        let year: u16 = dt.year().try_into().with_context(|_| ConversionSnafu {
            value: dt.year().to_string(),
            component: DateComponent::Year,
        })?;
        let month: u8 = dt.month().try_into().with_context(|_| ConversionSnafu {
            value: dt.month().to_string(),
            component: DateComponent::Month,
        })?;
        let day: u8 = dt.day().try_into().with_context(|_| ConversionSnafu {
            value: dt.day().to_string(),
            component: DateComponent::Day,
        })?;
        let hour: u8 = dt.hour().try_into().with_context(|_| ConversionSnafu {
            value: dt.hour().to_string(),
            component: DateComponent::Hour,
        })?;
        let minute: u8 = dt.minute().try_into().with_context(|_| ConversionSnafu {
            value: dt.minute().to_string(),
            component: DateComponent::Minute,
        })?;
        let second: u8 = dt.second().try_into().with_context(|_| ConversionSnafu {
            value: dt.second().to_string(),
            component: DateComponent::Second,
        })?;
        let microsecond = dt.nanosecond() / 1000;
        // leap second correction: convert (59, 1_000_000 + x) to (60, x)
        let (second, microsecond) = if microsecond >= 1_000_000 && second == 59 {
            (60, microsecond - 1_000_000)
        } else {
            (second, microsecond)
        };

        DicomDateTime::from_date_and_time_with_time_zone(
            DicomDate::from_ymd(year, month, day)?,
            DicomTime::from_hms_micro(hour, minute, second, microsecond)?,
            *dt.offset(),
        )
    }
}

impl TryFrom<&NaiveDateTime> for DicomDateTime {
    type Error = Error;
    fn try_from(dt: &NaiveDateTime) -> Result<Self> {
        let year: u16 = dt.year().try_into().with_context(|_| ConversionSnafu {
            value: dt.year().to_string(),
            component: DateComponent::Year,
        })?;
        let month: u8 = dt.month().try_into().with_context(|_| ConversionSnafu {
            value: dt.month().to_string(),
            component: DateComponent::Month,
        })?;
        let day: u8 = dt.day().try_into().with_context(|_| ConversionSnafu {
            value: dt.day().to_string(),
            component: DateComponent::Day,
        })?;
        let hour: u8 = dt.hour().try_into().with_context(|_| ConversionSnafu {
            value: dt.hour().to_string(),
            component: DateComponent::Hour,
        })?;
        let minute: u8 = dt.minute().try_into().with_context(|_| ConversionSnafu {
            value: dt.minute().to_string(),
            component: DateComponent::Minute,
        })?;
        let second: u8 = dt.second().try_into().with_context(|_| ConversionSnafu {
            value: dt.second().to_string(),
            component: DateComponent::Second,
        })?;
        let microsecond = dt.nanosecond() / 1000;
        // leap second correction: convert (59, 1_000_000 + x) to (60, x)
        let (second, microsecond) = if microsecond >= 1_000_000 && second == 59 {
            (60, microsecond - 1_000_000)
        } else {
            (second, microsecond)
        };

        DicomDateTime::from_date_and_time(
            DicomDate::from_ymd(year, month, day)?,
            DicomTime::from_hms_micro(hour, minute, second, microsecond)?,
        )
    }
}

impl fmt::Display for DicomDateTime {
    fn fmt(&self, frm: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.time {
            None => match self.time_zone {
                Some(offset) => write!(frm, "{} {}", self.date, offset),
                None => write!(frm, "{}", self.date),
            },
            Some(time) => match self.time_zone {
                Some(offset) => write!(frm, "{} {} {}", self.date, time, offset),
                None => write!(frm, "{} {}", self.date, time),
            },
        }
    }
}

impl fmt::Debug for DicomDateTime {
    fn fmt(&self, frm: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.time {
            None => match self.time_zone {
                Some(offset) => write!(frm, "{:?} {}", self.date, offset),
                None => write!(frm, "{:?}", self.date),
            },
            Some(time) => match self.time_zone {
                Some(offset) => write!(frm, "{:?} {:?} {}", self.date, time, offset),
                None => write!(frm, "{:?} {:?}", self.date, time),
            },
        }
    }
}

impl std::str::FromStr for DicomDateTime {
    type Err = crate::value::DeserializeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::value::deserialize::parse_datetime_partial(s.as_bytes())
    }
}

impl DicomDate {
    /**
     * Retrieves a dicom encoded string representation of the value.
     */
    pub fn to_encoded(&self) -> String {
        match self {
            DicomDate(DicomDateImpl::Year(y)) => format!("{y:04}"),
            DicomDate(DicomDateImpl::Month(y, m)) => format!("{y:04}{m:02}"),
            DicomDate(DicomDateImpl::Day(y, m, d)) => format!("{y:04}{m:02}{d:02}"),
        }
    }
}

impl DicomTime {
    /**
     * Retrieves a dicom encoded string representation of the value.
     */
    pub fn to_encoded(&self) -> String {
        match self {
            DicomTime(DicomTimeImpl::Hour(h)) => format!("{h:02}"),
            DicomTime(DicomTimeImpl::Minute(h, m)) => format!("{h:02}{m:02}"),
            DicomTime(DicomTimeImpl::Second(h, m, s)) => format!("{h:02}{m:02}{s:02}"),
            DicomTime(DicomTimeImpl::Fraction(h, m, s, f, fp)) => {
                let sfrac = (u32::pow(10, *fp as u32) + f).to_string();
                format!("{h:02}{m:02}{s:02}.{}", sfrac.get(1..).unwrap())
            }
        }
    }
}

impl DicomDateTime {
    /**
     * Retrieves a dicom encoded string representation of the value.
     */
    pub fn to_encoded(&self) -> String {
        match self.time {
            Some(time) => match self.time_zone {
                Some(offset) => format!(
                    "{}{}{}",
                    self.date.to_encoded(),
                    time.to_encoded(),
                    offset.to_string().replace(':', "")
                ),
                None => format!("{}{}", self.date.to_encoded(), time.to_encoded()),
            },
            None => match self.time_zone {
                Some(offset) => format!(
                    "{}{}",
                    self.date.to_encoded(),
                    offset.to_string().replace(':', "")
                ),
                None => self.date.to_encoded().to_string(),
            },
        }
    }
}

/// An encapsulated date-time value which is precise to the microsecond
/// and can either be time-zone aware or time-zone naive.
///
/// It is usually the outcome of converting a precise
/// [DICOM date-time value](DicomDateTime)
/// to a [chrono] date-time value.
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum PreciseDateTime {
    /// Naive date-time, with no time zone
    Naive(NaiveDateTime),
    /// Date-time with a time zone defined by a fixed offset
    TimeZone(DateTime<FixedOffset>),
}

impl PreciseDateTime {
    /// Retrieves a reference to a [`chrono::DateTime<FixedOffset>`][chrono::DateTime]
    /// if the result is time-zone aware.
    pub fn as_datetime(&self) -> Option<&DateTime<FixedOffset>> {
        match self {
            PreciseDateTime::Naive(..) => None,
            PreciseDateTime::TimeZone(value) => Some(value),
        }
    }

    /// Retrieves a reference to a [`chrono::NaiveDateTime`]
    /// only if the result is time-zone naive.
    pub fn as_naive_datetime(&self) -> Option<&NaiveDateTime> {
        match self {
            PreciseDateTime::Naive(value) => Some(value),
            PreciseDateTime::TimeZone(..) => None,
        }
    }

    /// Moves out a [`chrono::DateTime<FixedOffset>`](chrono::DateTime)
    /// if the result is time-zone aware.
    pub fn into_datetime(self) -> Option<DateTime<FixedOffset>> {
        match self {
            PreciseDateTime::Naive(..) => None,
            PreciseDateTime::TimeZone(value) => Some(value),
        }
    }

    /// Moves out a [`chrono::NaiveDateTime`]
    /// only if the result is time-zone naive.
    pub fn into_naive_datetime(self) -> Option<NaiveDateTime> {
        match self {
            PreciseDateTime::Naive(value) => Some(value),
            PreciseDateTime::TimeZone(..) => None,
        }
    }

    /// Retrieves the time-zone naive date component
    /// of the precise date-time value.
    ///
    /// # Panics
    ///
    /// The time-zone aware variant uses `DateTime`,
    /// which internally stores the date and time in UTC with a `NaiveDateTime`.
    /// This method will panic if the offset from UTC would push the local date
    /// outside of the representable range of a `NaiveDate`.
    pub fn to_naive_date(&self) -> NaiveDate {
        match self {
            PreciseDateTime::Naive(value) => value.date(),
            PreciseDateTime::TimeZone(value) => value.date_naive(),
        }
    }

    /// Retrieves the time component of the precise date-time value.
    pub fn to_naive_time(&self) -> NaiveTime {
        match self {
            PreciseDateTime::Naive(value) => value.time(),
            PreciseDateTime::TimeZone(value) => value.time(),
        }
    }

    /// Returns `true` if the result is time-zone aware.
    #[inline]
    pub fn has_time_zone(&self) -> bool {
        matches!(self, PreciseDateTime::TimeZone(..))
    }
}

/// The partial ordering for `PreciseDateTime`
/// is defined by the partial ordering of matching variants
/// (`Naive` with `Naive`, `TimeZone` with `TimeZone`).
///
/// Any other comparison cannot be defined,
/// and therefore will always return `None`.
impl PartialOrd for PreciseDateTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (PreciseDateTime::Naive(a), PreciseDateTime::Naive(b)) => a.partial_cmp(b),
            (PreciseDateTime::TimeZone(a), PreciseDateTime::TimeZone(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_dicom_date() {
        assert_eq!(
            DicomDate::from_ymd(1944, 2, 29).unwrap(),
            DicomDate(DicomDateImpl::Day(1944, 2, 29))
        );

        // cheap precision check, but date is invalid
        assert!(DicomDate::from_ymd(1945, 2, 29).unwrap().is_precise());
        assert_eq!(
            DicomDate::from_ym(1944, 2).unwrap(),
            DicomDate(DicomDateImpl::Month(1944, 2))
        );
        assert_eq!(
            DicomDate::from_y(1944).unwrap(),
            DicomDate(DicomDateImpl::Year(1944))
        );

        assert!(DicomDate::from_ymd(1944, 2, 29).unwrap().is_precise());
        assert!(!DicomDate::from_ym(1944, 2).unwrap().is_precise());
        assert!(!DicomDate::from_y(1944).unwrap().is_precise());
        assert_eq!(
            DicomDate::from_ymd(1944, 2, 29)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveDate::from_ymd_opt(1944, 2, 29).unwrap()
        );
        assert_eq!(
            DicomDate::from_ymd(1944, 2, 29).unwrap().latest().unwrap(),
            NaiveDate::from_ymd_opt(1944, 2, 29).unwrap()
        );

        assert_eq!(
            DicomDate::from_y(1944).unwrap().earliest().unwrap(),
            NaiveDate::from_ymd_opt(1944, 1, 1).unwrap()
        );
        // detects leap year
        assert_eq!(
            DicomDate::from_ym(1944, 2).unwrap().latest().unwrap(),
            NaiveDate::from_ymd_opt(1944, 2, 29).unwrap()
        );
        assert_eq!(
            DicomDate::from_ym(1945, 2).unwrap().latest().unwrap(),
            NaiveDate::from_ymd_opt(1945, 2, 28).unwrap()
        );

        assert_eq!(
            DicomDate::try_from(&NaiveDate::from_ymd_opt(1945, 2, 28).unwrap()).unwrap(),
            DicomDate(DicomDateImpl::Day(1945, 2, 28))
        );

        // date parsing

        let date: DicomDate = "20240229".parse().unwrap();
        assert_eq!(date, DicomDate(DicomDateImpl::Day(2024, 2, 29)));
        assert!(date.is_precise());

        // error cases

        assert!(matches!(
            DicomDate::try_from(&NaiveDate::from_ymd_opt(-2000, 2, 28).unwrap()),
            Err(Error::Conversion { .. })
        ));

        assert!(matches!(
            DicomDate::try_from(&NaiveDate::from_ymd_opt(10_000, 2, 28).unwrap()),
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
        // cheap precision checks
        assert!(DicomTime::from_hms_micro(9, 1, 1, 123456)
            .unwrap()
            .is_precise());
        assert!(!DicomTime::from_hms_milli(9, 1, 1, 123)
            .unwrap()
            .is_precise());

        assert_eq!(
            DicomTime::from_hms_milli(9, 1, 1, 123)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveTime::from_hms_micro_opt(9, 1, 1, 123_000).unwrap()
        );
        assert_eq!(
            DicomTime::from_hms_milli(9, 1, 1, 123)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveTime::from_hms_micro_opt(9, 1, 1, 123_999).unwrap()
        );

        assert_eq!(
            DicomTime::from_hms_milli(9, 1, 1, 2)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveTime::from_hms_micro_opt(9, 1, 1, /* 00 */ 2000).unwrap()
        );
        assert_eq!(
            DicomTime::from_hms_milli(9, 1, 1, 2)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveTime::from_hms_micro_opt(9, 1, 1, /* 00 */ 2999).unwrap()
        );

        assert!(DicomTime::from_hms_micro(9, 1, 1, 123456)
            .unwrap()
            .is_precise());

        assert_eq!(
            DicomTime::from_hms_milli(9, 1, 1, 1).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(9, 1, 1, 1, 3))
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_milli_opt(16, 31, 28, 123).unwrap()).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(16, 31, 28, 123_000, 6))
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro_opt(16, 31, 28, 123).unwrap()).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(16, 31, 28, /* 000 */ 123, 6))
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro_opt(16, 31, 28, 1234).unwrap()).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(16, 31, 28, /* 00 */ 1234, 6))
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro_opt(16, 31, 28, 0).unwrap()).unwrap(),
            DicomTime(DicomTimeImpl::Fraction(16, 31, 28, 0, 6))
        );

        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 1, 4).unwrap().to_string(),
            "09:01:01.0001"
        );
        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 0, 1).unwrap().to_string(),
            "09:01:01.0"
        );
        assert_eq!(
            DicomTime::from_hmsf(7, 55, 1, 1, 5).unwrap().to_encoded(),
            "075501.00001"
        );
        // the number of trailing zeros always complies with precision
        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 0, 2).unwrap().to_encoded(),
            "090101.00"
        );
        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 0, 3).unwrap().to_encoded(),
            "090101.000"
        );
        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 0, 4).unwrap().to_encoded(),
            "090101.0000"
        );
        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 0, 5).unwrap().to_encoded(),
            "090101.00000"
        );
        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 0, 6).unwrap().to_encoded(),
            "090101.000000"
        );

        // leap second allowed here
        assert_eq!(
            DicomTime::from_hmsf(23, 59, 60, 123, 3)
                .unwrap()
                .to_encoded(),
            "235960.123",
        );

        // leap second from chrono NaiveTime is admitted
        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro_opt(16, 31, 59, 1_000_000).unwrap())
                .unwrap()
                .to_encoded(),
            "163160.000000",
        );

        // sub-second precision after leap second from NaiveTime is admitted
        {
            let time =
                DicomTime::try_from(&NaiveTime::from_hms_micro_opt(16, 31, 59, 1_012_345).unwrap())
                    .unwrap();
            assert_eq!(time.to_encoded(), "163160.012345");

            assert_eq!(time.fraction_micro(), Some(12_345));
            assert_eq!(time.fraction_ms(), Some(12));
            assert_eq!(time.millisecond(), Some(12));
        }

        // time specifically with 0 microseconds
        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro_opt(16, 31, 59, 0).unwrap())
                .unwrap()
                .to_encoded(),
            "163159.000000",
        );

        // specific date-time from chrono
        let date_time: DateTime<_> = DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2024, 8, 9).unwrap(),
                NaiveTime::from_hms_opt(9, 9, 39).unwrap(),
            ),
            chrono::Utc,
        )
        .with_timezone(&FixedOffset::east_opt(0).unwrap());
        let dicom_date_time = DicomDateTime::try_from(&date_time).unwrap();
        assert!(dicom_date_time.has_time_zone());
        assert!(dicom_date_time.is_precise());
        let dicom_time = dicom_date_time.time().unwrap();
        assert_eq!(dicom_time.fraction_and_precision(), Some((0, 6)),);
        assert_eq!(dicom_date_time.to_encoded(), "20240809090939.000000+0000");

        assert_eq!(
            dicom_date_time.time().map(|t| t.millisecond()),
            Some(Some(0)),
        );

        // time parsing

        let time: DicomTime = "211133.7651".parse().unwrap();
        assert_eq!(
            time,
            DicomTime(DicomTimeImpl::Fraction(21, 11, 33, 7651, 4))
        );
        assert_eq!(time.fraction_ms(), Some(765));
        assert_eq!(time.fraction_micro(), Some(765_100));
        assert_eq!(time.fraction_precision(), 4);
        assert_eq!(&time.fraction_str(), "7651");

        // bad inputs

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

        // invalid second fraction: leap second not allowed here
        assert!(matches!(
            DicomTime::from_hmsf(9, 1, 1, 1_000_000, 6),
            Err(Error::InvalidComponent {
                component: DateComponent::Fraction,
                ..
            })
        ));

        assert!(matches!(
            DicomTime::from_hmsf(9, 1, 1, 12345, 5).unwrap().exact(),
            Err(crate::value::range::Error::ImpreciseValue { .. })
        ));

        // test fraction and precision - valid cases
        for (frac, frac_precision, microseconds) in [
            (1, 1, 100_000),
            (12, 2, 120_000),
            (123, 3, 123_000),
            (1234, 4, 123_400),
            (12345, 5, 123_450),
            (123456, 6, 123_456),
        ] {
            let time = DicomTime::from_hmsf(9, 1, 1, frac, frac_precision).unwrap();
            assert_eq!(time.fraction_micro(), Some(microseconds));
            assert_eq!(time.fraction_precision(), frac_precision);
            assert_eq!(time.fraction_str(), frac.to_string());
        }
        // test fraction retrieval: without a fraction, it returns None
        assert_eq!(DicomTime::from_hms(9, 1, 1).unwrap().fraction_micro(), None);
    }

    #[test]
    fn test_dicom_datetime() {
        let default_offset = FixedOffset::east_opt(0).unwrap();
        assert_eq!(
            DicomDateTime::from_date_with_time_zone(
                DicomDate::from_ymd(2020, 2, 29).unwrap(),
                default_offset
            ),
            DicomDateTime {
                date: DicomDate::from_ymd(2020, 2, 29).unwrap(),
                time: None,
                time_zone: Some(default_offset)
            }
        );

        assert_eq!(
            DicomDateTime::from_date(DicomDate::from_ym(2020, 2).unwrap())
                .earliest()
                .unwrap(),
            PreciseDateTime::Naive(NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2020, 2, 1).unwrap(),
                NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap()
            ))
        );

        assert_eq!(
            DicomDateTime::from_date_with_time_zone(
                DicomDate::from_ym(2020, 2).unwrap(),
                default_offset
            )
            .latest()
            .unwrap(),
            PreciseDateTime::TimeZone(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                        NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
                    ))
                    .unwrap()
            )
        );

        assert_eq!(
            DicomDateTime::from_date_and_time_with_time_zone(
                DicomDate::from_ymd(2020, 2, 29).unwrap(),
                DicomTime::from_hmsf(23, 59, 59, 10, 2).unwrap(),
                default_offset
            )
            .unwrap()
            .earliest()
            .unwrap(),
            PreciseDateTime::TimeZone(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                        NaiveTime::from_hms_micro_opt(23, 59, 59, 100_000).unwrap()
                    ))
                    .unwrap()
            )
        );
        assert_eq!(
            DicomDateTime::from_date_and_time_with_time_zone(
                DicomDate::from_ymd(2020, 2, 29).unwrap(),
                DicomTime::from_hmsf(23, 59, 59, 10, 2).unwrap(),
                default_offset
            )
            .unwrap()
            .latest()
            .unwrap(),
            PreciseDateTime::TimeZone(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                        NaiveTime::from_hms_micro_opt(23, 59, 59, 109_999).unwrap()
                    ))
                    .unwrap()
            )
        );

        assert_eq!(
            DicomDateTime::try_from(
                &FixedOffset::east_opt(0)
                    .unwrap()
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                        NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
                    ))
                    .unwrap()
            )
            .unwrap(),
            DicomDateTime {
                date: DicomDate::from_ymd(2020, 2, 29).unwrap(),
                time: Some(DicomTime::from_hms_micro(23, 59, 59, 999_999).unwrap()),
                time_zone: Some(default_offset)
            }
        );

        assert_eq!(
            DicomDateTime::try_from(
                &FixedOffset::east_opt(0)
                    .unwrap()
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(2020, 2, 29).unwrap(),
                        NaiveTime::from_hms_micro_opt(23, 59, 59, 0).unwrap()
                    ))
                    .unwrap()
            )
            .unwrap(),
            DicomDateTime {
                date: DicomDate::from_ymd(2020, 2, 29).unwrap(),
                time: Some(DicomTime::from_hms_micro(23, 59, 59, 0).unwrap()),
                time_zone: Some(default_offset)
            }
        );

        // leap second from chrono NaiveTime is admitted
        assert_eq!(
            DicomDateTime::try_from(
                &FixedOffset::east_opt(0)
                    .unwrap()
                    .from_local_datetime(&NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
                        NaiveTime::from_hms_micro_opt(23, 59, 59, 1_000_000).unwrap()
                    ))
                    .unwrap()
            )
            .unwrap(),
            DicomDateTime {
                date: DicomDate::from_ymd(2023, 12, 31).unwrap(),
                time: Some(DicomTime::from_hms_micro(23, 59, 60, 0).unwrap()),
                time_zone: Some(default_offset)
            }
        );

        // date-time parsing

        let dt: DicomDateTime = "20240229235959.123456+0000".parse().unwrap();
        assert_eq!(
            dt,
            DicomDateTime {
                date: DicomDate::from_ymd(2024, 2, 29).unwrap(),
                time: Some(DicomTime::from_hms_micro(23, 59, 59, 123_456).unwrap()),
                time_zone: Some(default_offset)
            }
        );
        assert!(dt.is_precise());

        // error cases

        assert!(matches!(
            DicomDateTime::from_date_with_time_zone(
                DicomDate::from_ymd(2021, 2, 29).unwrap(),
                default_offset
            )
            .earliest(),
            Err(crate::value::range::Error::InvalidDate { .. })
        ));

        assert!(matches!(
            DicomDateTime::from_date_and_time_with_time_zone(
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
            DicomDateTime::from_date_and_time_with_time_zone(
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
            DicomDateTime::from_date_and_time_with_time_zone(
                DicomDate::from_ymd(2000, 1, 1).unwrap(),
                DicomTime::from_hms_milli(23, 59, 59, 10).unwrap(),
                default_offset
            )
            .unwrap()
            .exact(),
            Err(crate::value::range::Error::ImpreciseValue { .. })
        ));

        // simple precision checks
        assert!(!DicomDateTime::from_date_and_time(
            DicomDate::from_ymd(2000, 1, 1).unwrap(),
            DicomTime::from_hms_milli(23, 59, 59, 10).unwrap()
        )
        .unwrap()
        .is_precise());

        assert!(DicomDateTime::from_date_and_time(
            DicomDate::from_ymd(2000, 1, 1).unwrap(),
            DicomTime::from_hms_micro(23, 59, 59, 654_321).unwrap()
        )
        .unwrap()
        .is_precise());
    }
}
