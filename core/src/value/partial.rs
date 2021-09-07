// Handling of partial precision of Date, Time and DateTime values.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use snafu::{Backtrace, OptionExt, Snafu};
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
    #[snafu(display("To combine a PartialDate with a PartialTime value, the PartialDate has to be precise. Precision is: '{:?}'.", value))]
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
}

type Result<T, E = Error> = std::result::Result<T, E>;

/**
 * Represents components of Date, Time and DateTime values.
 */
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

/**
 * Represents a Dicom Date value with a partial precision,
 * where some time components may be missing.
 */
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PartialDate {
    Year(u16),
    Month(u16, u8),
    Day(u16, u8, u8),
}

/**
 * Represents a Dicom Time value with a partial precision,
 * where some time components may be missing.
 */
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PartialTime {
    Hour(u8),
    Minute(u8, u8),
    Second(u8, u8, u8),
    Fraction(u8, u8, u8, u32, u8),
}

/**
 * Represents a Dicom DateTime value with a partial precision,
 * where some time components may be missing.
 */
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct PartialDateTime {
    date: PartialDate,
    time: Option<PartialTime>,
    offset: FixedOffset,
}

/**
 * Throws a detailed `InvalidComponent` error if Date / Time components are out of range.
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

impl PartialDate {
    /**
     * Constructs a new `PartialDate` with year precision
     * (YYYY)
     */
    pub fn from_y(year: u16) -> Result<PartialDate> {
        check_component(DateComponent::Year, &year)?;
        Ok(PartialDate::Year(year))
    }
    /**
     * Constructs a new `PartialDate` with year and month precision
     * (YYYYMM)
     */
    pub fn from_ym(year: u16, month: u8) -> Result<PartialDate> {
        check_component(DateComponent::Year, &year)?;
        check_component(DateComponent::Month, &month)?;
        Ok(PartialDate::Month(year, month))
    }
    /**
     * Constructs a new `PartialDate` with a year, month and day precision
     * (YYYYMMDD)
     */
    pub fn from_ymd(year: u16, month: u8, day: u8) -> Result<PartialDate> {
        check_component(DateComponent::Year, &year)?;
        check_component(DateComponent::Month, &month)?;
        check_component(DateComponent::Day, &day)?;
        Ok(PartialDate::Day(year, month, day))
    }
}

impl PartialTime {
    /**
     * Constructs a new `PartialTime` with hour precision
     * (HH)
     */
    pub fn from_h(hour: u8) -> Result<PartialTime> {
        check_component(DateComponent::Hour, &hour)?;
        Ok(PartialTime::Hour(hour))
    }

    /**
     * Constructs a new `PartialTime` with hour and minute precision
     * (HHMM)
     */
    pub fn from_hm(hour: u8, minute: u8) -> Result<PartialTime> {
        check_component(DateComponent::Hour, &hour)?;
        check_component(DateComponent::Minute, &minute)?;
        Ok(PartialTime::Minute(hour, minute))
    }

    /**
     * Constructs a new `PartialTime` with hour, minute and second precision
     * (HHMMSS)
     */
    pub fn from_hms(hour: u8, minute: u8, second: u8) -> Result<PartialTime> {
        check_component(DateComponent::Hour, &hour)?;
        check_component(DateComponent::Minute, &minute)?;
        check_component(DateComponent::Second, &second)?;
        Ok(PartialTime::Second(hour, minute, second))
    }

    /**
     * Constructs a new `PartialTime` with hour, minute, second and second fraction
     * precision (HHMMSS.FFFFFF).
     */
    pub fn from_hmsf(
        hour: u8,
        minute: u8,
        second: u8,
        fraction: u32,
        frac_precision: u8,
    ) -> Result<PartialTime> {
        if !(1..=6).contains(&frac_precision) {
            return FractionPrecisionRange {
                value: frac_precision,
            }
            .fail();
        }
        if u32::pow(10, frac_precision as u32) < fraction {
            return FractionPrecisionMismatch {
                fraction: fraction,
                precision: frac_precision,
            }
            .fail();
        }

        check_component(DateComponent::Hour, &hour)?;
        check_component(DateComponent::Minute, &minute)?;
        check_component(DateComponent::Second, &second)?;
        let f: u32 = fraction * u32::pow(10, 6 - frac_precision as u32);
        check_component(DateComponent::Fraction, &f)?;
        Ok(PartialTime::Fraction(
            hour,
            minute,
            second,
            fraction,
            frac_precision,
        ))
    }
}

impl PartialDateTime {
    /**
     * Constructs a new `PartialDateTime` from a `PartialDate` and a given `FixedOffset`.
     */
    pub fn from_partial_date(date: PartialDate, offset: FixedOffset) -> PartialDateTime {
        PartialDateTime {
            date,
            time: None,
            offset,
        }
    }

    /**
     * Constructs a new `PartialDateTime` from a `PartialDate`, `PartialTime` and a given `FixedOffset`,
     * providing that `PartialDate.is_precise() == true`.
     */
    pub fn from_partial_date_and_time(
        date: PartialDate,
        time: PartialTime,
        offset: FixedOffset,
    ) -> Result<PartialDateTime> {
        if date.is_precise() {
            Ok(PartialDateTime {
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

/**
 * This trait is implemented by partial precision
 * Date, Time and DateTime structures.
 * Trait method returns the last fully precise `DateComponent` of the structure.
 */
pub trait Precision {
    fn precision(&self) -> DateComponent;
}

impl Precision for PartialDate {
    fn precision(&self) -> DateComponent {
        match self {
            PartialDate::Year(..) => DateComponent::Year,
            PartialDate::Month(..) => DateComponent::Month,
            PartialDate::Day(..) => DateComponent::Day,
        }
    }
}

impl Precision for PartialTime {
    fn precision(&self) -> DateComponent {
        match self {
            PartialTime::Hour(..) => DateComponent::Hour,
            PartialTime::Minute(..) => DateComponent::Minute,
            PartialTime::Second(..) => DateComponent::Second,
            PartialTime::Fraction(..) => DateComponent::Fraction,
        }
    }
}

impl Precision for PartialDateTime {
    fn precision(&self) -> DateComponent {
        if self.time.is_some() {
            self.time.unwrap().precision()
        } else {
            self.date.precision()
        }
    }
}

/**
 * This trait is implemented by partial precision Date, Time and DateTime structures.
 * Unlike RUSTs `chrono`, this implemenation of Date / Time is DICOM compliant:  
 * has only 6 digit precision for fracion of a second
 * and has no means to handle leap seconds
 */
pub trait AsTemporalRange<T>: Precision
where
    T: PartialEq,
{
    /**
     * Returns the earliest possible value from a partial precision structure.
     * So missing components default to 1 (days, months) or 0 (hours, minutes, ...)
     * If structure contains invalid combination of `DateComponent`s, it fails.
     */
    fn earliest(&self) -> Result<T>;

    /**
     * Returns the latest possible value from a partial precision structure.
     * If structure contains invalid combination of `DateComponent`s, it fails.
     */
    fn latest(&self) -> Result<T>;

    /**
     * Returns a tuple of the earliest and latest possible value from a partial precision structure.
     *
     */
    fn to_range(&self) -> Result<(Option<T>, Option<T>)> {
        Ok((self.earliest().ok(), self.latest().ok()))
    }

    /**
     * Returns `true`, if partial precision structure has maximum possible accuracy.
     * For fraction of a second, only a 6 digit precision returns `true`.
     */
    fn is_precise(&self) -> bool {
        let e = self.earliest();
        let l = self.latest();

        e.is_ok() && l.is_ok() && e.ok() == l.ok()
    }
}

impl AsTemporalRange<NaiveDate> for PartialDate {
    fn earliest(&self) -> Result<NaiveDate> {
        let (y, m, d) = match self {
            PartialDate::Year(y) => (*y as i32, 1, 1),
            PartialDate::Month(y, m) => (*y as i32, *m as u32, 1),
            PartialDate::Day(y, m, d) => (*y as i32, *m as u32, *d as u32),
        };
        NaiveDate::from_ymd_opt(y, m, d).context(InvalidDate)
    }

    fn latest(&self) -> Result<NaiveDate> {
        let (y, m, d) = match self {
            PartialDate::Year(y) => (*y as i32, 12, 31),
            PartialDate::Month(y, m) => {
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
            PartialDate::Day(y, m, d) => (*y as i32, *m as u32, *d as u32),
        };
        NaiveDate::from_ymd_opt(y, m, d).context(InvalidDate)
    }
}

impl AsTemporalRange<NaiveTime> for PartialTime {
    fn earliest(&self) -> Result<NaiveTime> {
        let fr: u32;
        let (h, m, s, f) = match self {
            PartialTime::Hour(h) => (h, &0, &0, &0),
            PartialTime::Minute(h, m) => (h, m, &0, &0),
            PartialTime::Second(h, m, s) => (h, m, s, &0),
            PartialTime::Fraction(h, m, s, f, fp) => {
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
            PartialTime::Hour(h) => (h, &59, &59, &999_999),
            PartialTime::Minute(h, m) => (h, m, &59, &999_999),
            PartialTime::Second(h, m, s) => (h, m, s, &999_999),
            PartialTime::Fraction(h, m, s, f, fp) => {
                fr = (*f * u32::pow(10, 6 - u32::from(*fp))) + (u32::pow(10, 6 - u32::from(*fp)))
                    - 1;
                (h, m, s, &fr)
            }
        };
        NaiveTime::from_hms_micro_opt((*h).into(), (*m).into(), (*s).into(), *f)
            .context(InvalidTime)
    }
}

impl AsTemporalRange<DateTime<FixedOffset>> for PartialDateTime {
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
    fn test_partial_date() {
        assert_eq!(
            PartialDate::from_ymd(1944, 2, 29).unwrap(),
            PartialDate::Day(1944, 2, 29)
        );
        assert_eq!(
            PartialDate::from_ym(1944, 2).unwrap(),
            PartialDate::Month(1944, 2)
        );
        assert_eq!(PartialDate::from_y(1944).unwrap(), PartialDate::Year(1944));

        assert_eq!(
            PartialDate::from_ymd(1944, 2, 29).unwrap().is_precise(),
            true
        );
        assert_eq!(PartialDate::from_ym(1944, 2).unwrap().is_precise(), false);
        assert_eq!(PartialDate::from_y(1944).unwrap().is_precise(), false);
        assert_eq!(
            PartialDate::from_ymd(1944, 2, 29)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveDate::from_ymd(1944, 2, 29)
        );
        assert_eq!(
            PartialDate::from_ymd(1944, 2, 29)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveDate::from_ymd(1944, 2, 29)
        );

        assert_eq!(
            PartialDate::from_y(1944).unwrap().earliest().unwrap(),
            NaiveDate::from_ymd(1944, 1, 1)
        );
        // detects leap year
        assert_eq!(
            PartialDate::from_ym(1944, 2).unwrap().latest().unwrap(),
            NaiveDate::from_ymd(1944, 2, 29)
        );
        assert_eq!(
            PartialDate::from_ym(1945, 2).unwrap().latest().unwrap(),
            NaiveDate::from_ymd(1945, 2, 28)
        );
    }

    #[test]
    fn test_partial_time() {
        assert_eq!(
            PartialTime::from_hmsf(9, 1, 1, 123456, 6).unwrap(),
            PartialTime::Fraction(9, 1, 1, 123456, 6)
        );
        assert_eq!(
            PartialTime::from_hmsf(9, 1, 1, 1, 6).unwrap(),
            PartialTime::Fraction(9, 1, 1, 1, 6)
        );
        assert_eq!(
            PartialTime::from_hms(9, 0, 0).unwrap(),
            PartialTime::Second(9, 0, 0)
        );
        assert_eq!(
            PartialTime::from_hm(23, 59).unwrap(),
            PartialTime::Minute(23, 59)
        );
        assert_eq!(PartialTime::from_h(1).unwrap(), PartialTime::Hour(1));

        assert_eq!(
            PartialTime::from_hmsf(9, 1, 1, 123, 3)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 123_000)
        );
        assert_eq!(
            PartialTime::from_hmsf(9, 1, 1, 123, 3)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 123_999)
        );

        assert_eq!(
            PartialTime::from_hmsf(9, 1, 1, 1, 1)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 100_000)
        );
        assert_eq!(
            PartialTime::from_hmsf(9, 1, 1, 1, 1)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 199_999)
        );

        assert_eq!(
            PartialTime::from_hmsf(9, 1, 1, 12345, 5)
                .unwrap()
                .is_precise(),
            false
        );

        assert_eq!(
            PartialTime::from_hmsf(9, 1, 1, 123456, 6)
                .unwrap()
                .is_precise(),
            true
        );

        assert!(matches!(
            PartialTime::from_hmsf(9, 1, 1, 1, 7),
            Err(Error::FractionPrecisionRange { value: 7, .. })
        ));

        assert!(matches!(
            PartialTime::from_hmsf(9, 1, 1, 123456, 3),
            Err(Error::FractionPrecisionMismatch {
                fraction: 123456,
                precision: 3,
                ..
            })
        ));
    }

    #[test]
    fn test_partial_datetime() {
        let default_offset = FixedOffset::east(0);
        assert_eq!(
            PartialDateTime::from_partial_date(
                PartialDate::from_ymd(2020, 2, 29).unwrap(),
                default_offset
            ),
            PartialDateTime {
                date: PartialDate::from_ymd(2020, 2, 29).unwrap(),
                time: None,
                offset: default_offset
            }
        );

        assert_eq!(
            PartialDateTime::from_partial_date(
                PartialDate::from_ym(2020, 2).unwrap(),
                default_offset
            )
            .earliest()
            .unwrap(),
            FixedOffset::east(0)
                .ymd(2020, 2, 1)
                .and_hms_micro(0, 0, 0, 0)
        );

        assert_eq!(
            PartialDateTime::from_partial_date(
                PartialDate::from_ym(2020, 2).unwrap(),
                default_offset
            )
            .latest()
            .unwrap(),
            FixedOffset::east(0)
                .ymd(2020, 2, 29)
                .and_hms_micro(23, 59, 59, 999_999)
        );

        assert_eq!(
            PartialDateTime::from_partial_date_and_time(
                PartialDate::from_ymd(2020, 2, 29).unwrap(),
                PartialTime::from_hmsf(23, 59, 59, 10, 2).unwrap(),
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
            PartialDateTime::from_partial_date_and_time(
                PartialDate::from_ymd(2020, 2, 29).unwrap(),
                PartialTime::from_hmsf(23, 59, 59, 10, 2).unwrap(),
                default_offset
            )
            .unwrap()
            .latest()
            .unwrap(),
            FixedOffset::east(0)
                .ymd(2020, 2, 29)
                .and_hms_micro(23, 59, 59, 109_999)
        );

        assert!(matches!(
            PartialDateTime::from_partial_date(
                PartialDate::from_ymd(2021, 2, 29).unwrap(),
                default_offset
            )
            .earliest(),
            Err(Error::InvalidDate { .. })
        ));

        assert!(matches!(
            PartialDateTime::from_partial_date_and_time(
                PartialDate::from_ym(2020, 2).unwrap(),
                PartialTime::from_hmsf(23, 59, 59, 10, 2).unwrap(),
                default_offset
            ),
            Err(Error::DateTimeFromPartials {
                value: DateComponent::Month,
                ..
            })
        ));
        assert!(matches!(
            PartialDateTime::from_partial_date_and_time(
                PartialDate::from_y(1).unwrap(),
                PartialTime::from_hmsf(23, 59, 59, 10, 2).unwrap(),
                default_offset
            ),
            Err(Error::DateTimeFromPartials {
                value: DateComponent::Year,
                ..
            })
        ));
    }
}
