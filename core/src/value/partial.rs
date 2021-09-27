//! Handling of partial precision of Date, Time and DateTime values.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveTime, TimeZone, Timelike};
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::convert::{TryFrom, TryInto};
use std::ops::RangeInclusive;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Date is invalid."))]
    InvalidDate { backtrace: Backtrace },
    #[snafu(display("Time is invalid."))]
    InvalidTime { backtrace: Backtrace },
    #[snafu(display("DateTime is invalid."))]
    InvalidDateTime { backtrace: Backtrace },
    #[snafu(display("To combine a DicomDate with a DicomTime value, the DicomDate has to be precise. Precision is: '{:?}'.", value))]
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
        "Second fraction precision '{}' is out of range, must be in 0..=6.",
        value
    ))]
    FractionPrecisionRange { value: u32, backtrace: Backtrace },
    #[snafu(display(
        "Number of digits in decimal representation of fraction '{}' does not match it's precision '{}'.",
        fraction,
        precision
    ))]
    FractionPrecisionMismatch {
        fraction: u32,
        precision: u32,
        backtrace: Backtrace,
    },
    #[snafu(display("Conversion Failed."))]
    Conversion { source: std::num::TryFromIntError },
    #[snafu(display(
        "Cannot convert from an imprecise value. This value represents a date / time range."
    ))]
    ImpreciseValue { backtrace: Backtrace },
}

type Result<T, E = Error> = std::result::Result<T, E>;

/// Represents components of Date, Time and DateTime values.
#[derive(Debug, PartialEq)]
pub enum DateComponent {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Fraction,
    UTCOffset,
}


/// Represents a Dicom Date value with a partial precision,
/// where some date components may be missing.
/// 
/// Unlike RUSTs `chrono::NaiveDate`, it does not allow for negative years.
///
/// Although the DICOM protocol does not allow for an incomplete DA value,
/// this structure is necessary for range matching purposes, where incomplete
/// date values occur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DicomDate {
    Year(u16),
    Month(u16, u8),
    Day(u16, u8, u8),
}


/// Represents a Dicom Time value with a partial precision,
/// where some time components may be missing.
/// 
/// Unlike RUSTs `chrono::NaiveTime`, this implemenation of time is DICOM compliant:  
/// - has only 6 digit precision for fraction of a second
/// - has no means to handle leap seconds
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DicomTime {
    Hour(u8),
    Minute(u8, u8),
    Second(u8, u8, u8),
    Fraction(u8, u8, u8, u32, u8),
}


/// Represents a Dicom DateTime value with a partial precision,
/// where some date / time components may be missing.
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
        DateComponent::Second => 0..=59,
        DateComponent::Fraction => 0..=999_999,
        DateComponent::UTCOffset => 0..=86_399,
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
        Ok(DicomDate::Year(year))
    }
    /**
     * Constructs a new `DicomDate` with year and month precision
     * (YYYYMM)
     */
    pub fn from_ym(year: u16, month: u8) -> Result<DicomDate> {
        check_component(DateComponent::Year, &year)?;
        check_component(DateComponent::Month, &month)?;
        Ok(DicomDate::Month(year, month))
    }
    /**
     * Constructs a new `DicomDate` with a year, month and day precision
     * (YYYYMMDD)
     */
    pub fn from_ymd(year: u16, month: u8, day: u8) -> Result<DicomDate> {
        check_component(DateComponent::Year, &year)?;
        check_component(DateComponent::Month, &month)?;
        check_component(DateComponent::Day, &day)?;
        Ok(DicomDate::Day(year, month, day))
    }
}

impl TryFrom<&NaiveDate> for DicomDate {
    type Error = Error;
    fn try_from(date: &NaiveDate) -> Result<Self> {
        let year: u16 = date.year().try_into().context(Conversion)?;
        let month: u8 = date.month().try_into().context(Conversion)?;
        let day: u8 = date.day().try_into().context(Conversion)?;
        Ok(DicomDate::from_ymd(year, month, day)?)
    }
}

impl DicomTime {
    /**
     * Constructs a new `DicomTime` with hour precision
     * (HH).
     */
    pub fn from_h(hour: u8) -> Result<DicomTime> {
        check_component(DateComponent::Hour, &hour)?;
        Ok(DicomTime::Hour(hour))
    }

    /**
     * Constructs a new `DicomTime` with hour and minute precision
     * (HHMM).
     */
    pub fn from_hm(hour: u8, minute: u8) -> Result<DicomTime> {
        check_component(DateComponent::Hour, &hour)?;
        check_component(DateComponent::Minute, &minute)?;
        Ok(DicomTime::Minute(hour, minute))
    }

    /**
     * Constructs a new `DicomTime` with hour, minute and second precision
     * (HHMMSS).
     */
    pub fn from_hms(hour: u8, minute: u8, second: u8) -> Result<DicomTime> {
        check_component(DateComponent::Hour, &hour)?;
        check_component(DateComponent::Minute, &minute)?;
        check_component(DateComponent::Second, &second)?;
        Ok(DicomTime::Second(hour, minute, second))
    }

    /**
     * Constructs a new `DicomTime` with hour, minute, second and second fraction
     * precision (HHMMSS.FFFFFF).
     */
    pub fn from_hmsf(
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
        Ok(DicomTime::Fraction(
            hour,
            minute,
            second,
            fraction,
            frac_precision,
        ))
    }
}

impl TryFrom<&NaiveTime> for DicomTime {
    type Error = Error;
    fn try_from(time: &NaiveTime) -> Result<Self> {
        let hour: u8 = time.hour().try_into().context(Conversion)?;
        let minute: u8 = time.minute().try_into().context(Conversion)?;
        let second: u8 = time.second().try_into().context(Conversion)?;
        let fraction: u32 = time.nanosecond() / 1000;
        // conversion from NaiveTime always leads to full precision (6)
        Ok(DicomTime::from_hmsf(hour, minute, second, fraction, 6)?)
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
        let year: u16 = dt.year().try_into().context(Conversion)?;
        let month: u8 = dt.month().try_into().context(Conversion)?;
        let day: u8 = dt.day().try_into().context(Conversion)?;
        let hour: u8 = dt.hour().try_into().context(Conversion)?;
        let minute: u8 = dt.minute().try_into().context(Conversion)?;
        let second: u8 = dt.second().try_into().context(Conversion)?;
        let fraction: u32 = dt.nanosecond() / 1000;

        Ok(DicomDateTime::from_dicom_date_and_time(
            DicomDate::from_ymd(year, month, day)?,
            DicomTime::from_hmsf(hour, minute, second, fraction, 6)?,
            *dt.offset(),
        )?)
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
            DicomDate::Year(..) => DateComponent::Year,
            DicomDate::Month(..) => DateComponent::Month,
            DicomDate::Day(..) => DateComponent::Day,
        }
    }
}

impl Precision for DicomTime {
    fn precision(&self) -> DateComponent {
        match self {
            DicomTime::Hour(..) => DateComponent::Hour,
            DicomTime::Minute(..) => DateComponent::Minute,
            DicomTime::Second(..) => DateComponent::Second,
            DicomTime::Fraction(..) => DateComponent::Fraction,
        }
    }
}

impl Precision for DicomDateTime {
    fn precision(&self) -> DateComponent {
        if self.time.is_some() {
            self.time.unwrap().precision()
        } else {
            self.date.precision()
        }
    }
}

/// The DICOM protocol accepts date / time values with null components.
/// Missing components are to be handled as date / time ranges.
/// This trait is implemented by date / time structures with partial precision.
///
/// - `.exact()` - Returns a corresponding `chrono` value, if the partial precision structure has full accuracy.
/// - `.earliest()` - Returns the earliest possible `chrono` value from a partial precision structure.
/// - `.latest()` - Returns the latest possible `chrono` value from a partial precision structure.
/// - `.range()` - Returns a range from earliest to latest possible `chrono` value.
/// - `.is_precise()` - Returns `true`, if partial precision structure has maximum possible accuracy.
pub trait AsRange<T>: Precision
where
    T: PartialEq,
{
    /**
     * Returns the earliest possible `chrono` value from a partial precision structure.
     * Missing components default to 1 (days, months) or 0 (hours, minutes, ...)
     * If structure contains invalid combination of `DateComponent`s, it fails.
     */
    fn earliest(&self) -> Result<T>;

    /**
     * Returns the latest possible `chrono` value from a partial precision structure.
     * If structure contains invalid combination of `DateComponent`s, it fails.
     */
    fn latest(&self) -> Result<T>;

    /**
     * Returns a tuple of the earliest and latest possible value from a partial precision structure.
     *
     */
    fn range(&self) -> Result<(Option<T>, Option<T>)> {
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

    // this one needs to be reimplemented for DicomTime, as to loosen the strict requirement
    // of 6 digits fraction accuracy
    /**
     * Returns a corresponding `chrono` value, if the partial precision structure has full accuracy.
     */
    fn exact(&self) -> Result<T> {
        if self.is_precise() {
            Ok(self.earliest()?)
        } else {
            ImpreciseValue.fail()
        }
    }
}

impl AsRange<NaiveDate> for DicomDate {
    fn earliest(&self) -> Result<NaiveDate> {
        let (y, m, d) = match self {
            DicomDate::Year(y) => (*y as i32, 1, 1),
            DicomDate::Month(y, m) => (*y as i32, *m as u32, 1),
            DicomDate::Day(y, m, d) => (*y as i32, *m as u32, *d as u32),
        };
        NaiveDate::from_ymd_opt(y, m, d).context(InvalidDate)
    }

    fn latest(&self) -> Result<NaiveDate> {
        let (y, m, d) = match self {
            DicomDate::Year(y) => (*y as i32, 12, 31),
            DicomDate::Month(y, m) => {
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
            DicomDate::Day(y, m, d) => (*y as i32, *m as u32, *d as u32),
        };
        NaiveDate::from_ymd_opt(y, m, d).context(InvalidDate)
    }
}

impl AsRange<NaiveTime> for DicomTime {
    fn earliest(&self) -> Result<NaiveTime> {
        let fr: u32;
        let (h, m, s, f) = match self {
            DicomTime::Hour(h) => (h, &0, &0, &0),
            DicomTime::Minute(h, m) => (h, m, &0, &0),
            DicomTime::Second(h, m, s) => (h, m, s, &0),
            DicomTime::Fraction(h, m, s, f, fp) => {
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
            DicomTime::Hour(h) => (h, &59, &59, &999_999),
            DicomTime::Minute(h, m) => (h, m, &59, &999_999),
            DicomTime::Second(h, m, s) => (h, m, s, &999_999),
            DicomTime::Fraction(h, m, s, f, fp) => {
                fr = (*f * u32::pow(10, 6 - u32::from(*fp))) + (u32::pow(10, 6 - u32::from(*fp)))
                    - 1;
                (h, m, s, &fr)
            }
        };
        NaiveTime::from_hms_micro_opt((*h).into(), (*m).into(), (*s).into(), *f)
            .context(InvalidTime)
    }

    // If fraction with any accuracy is present, `DicomTime` is handled as precise.
    // missing accuracy falls back to zeros.
    fn exact(&self) -> Result<NaiveTime> {
        match self {
            DicomTime::Fraction(..) => Ok(self.earliest()?),
            _ => ImpreciseValue.fail(),
        }
    }
}

impl AsRange<DateTime<FixedOffset>> for DicomDateTime {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dicom_date() {
        assert_eq!(
            DicomDate::from_ymd(1944, 2, 29).unwrap(),
            DicomDate::Day(1944, 2, 29)
        );
        assert_eq!(
            DicomDate::from_ym(1944, 2).unwrap(),
            DicomDate::Month(1944, 2)
        );
        assert_eq!(DicomDate::from_y(1944).unwrap(), DicomDate::Year(1944));

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
            DicomDate::Day(1945, 2, 28)
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
            DicomTime::from_hmsf(9, 1, 1, 123456, 6).unwrap(),
            DicomTime::Fraction(9, 1, 1, 123456, 6)
        );
        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 1, 6).unwrap(),
            DicomTime::Fraction(9, 1, 1, 1, 6)
        );
        assert_eq!(
            DicomTime::from_hms(9, 0, 0).unwrap(),
            DicomTime::Second(9, 0, 0)
        );
        assert_eq!(
            DicomTime::from_hm(23, 59).unwrap(),
            DicomTime::Minute(23, 59)
        );
        assert_eq!(DicomTime::from_h(1).unwrap(), DicomTime::Hour(1));

        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 123, 3)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 123_000)
        );
        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 123, 3)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 123_999)
        );

        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 1, 1)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 100_000)
        );
        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 1, 1)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 199_999)
        );

        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 12345, 5)
                .unwrap()
                .is_precise(),
            false
        );

        assert_eq!(
            DicomTime::from_hmsf(9, 1, 1, 123456, 6)
                .unwrap()
                .is_precise(),
            true
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_milli(16, 31, 28, 123)).unwrap(),
            DicomTime::Fraction(16, 31, 28, 123_000, 6)
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro(16, 31, 28, 123)).unwrap(),
            DicomTime::Fraction(16, 31, 28, 000123, 6)
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro(16, 31, 28, 1234)).unwrap(),
            DicomTime::Fraction(16, 31, 28, 001234, 6)
        );

        assert_eq!(
            DicomTime::try_from(&NaiveTime::from_hms_micro(16, 31, 28, 0)).unwrap(),
            DicomTime::Fraction(16, 31, 28, 0, 6)
        );

        assert!(matches!(
            DicomTime::from_hmsf(9, 1, 1, 1, 7),
            Err(Error::FractionPrecisionRange { value: 7, .. })
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
                time: Some(DicomTime::from_hmsf(23, 59, 59, 999_999, 6).unwrap()),
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
                time: Some(DicomTime::from_hmsf(23, 59, 59, 0, 6).unwrap()),
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
                DicomTime::from_hmsf(23, 59, 59, 10, 2).unwrap(),
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
                DicomTime::from_hmsf(23, 59, 59, 10, 2).unwrap(),
                default_offset
            ),
            Err(Error::DateTimeFromPartials {
                value: DateComponent::Year,
                ..
            })
        ));
    }
}
