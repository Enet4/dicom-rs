// Handling of partial precision of Date, Time and DateTime values.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::ops::RangeInclusive;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Date is invalid."))]
    InvalidDate { backtrace: Backtrace },
    #[snafu(display("Time is invalid."))]
    InvalidTime { backtrace: Backtrace },
    #[snafu(display(
        "{:?} has invalid value:  {}, must be in {:?}",
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
#[derive(Debug, PartialEq)]
pub enum PartialDate {
    Year(u16),
    Month(u16, u8),
    Day(u16, u8, u8),
}

/**
 * Represents a Dicom Time value with a partial precision,
 * where some time components may be missing.
 */
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
pub enum PartialDateTime {
    Year(u16),
    Month(u16, u8),
    Day(u16, u8, u8),
    Hour(u16, u8, u8, u8),
    Minute(u16, u8, u8, u8, u8),
    Second(u16, u8, u8, u8, u8, u8),
    Fraction(u16, u8, u8, u8, u8, u8, u32),
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
    pub fn from_y<T>(y: &T) -> Result<PartialDate>
    where
        T: Into<u32> + Into<u16> + Copy,
    {
        check_component(DateComponent::Year, y)?;
        Ok(PartialDate::Year((*y).into()))
    }
    /**
     * Constructs a new `PartialDate` with year and month precision
     * (YYYYMM)
     */
    pub fn from_ym<T, U>(y: &T, m: &U) -> Result<PartialDate>
    where
        T: Into<u32> + Into<u16> + Copy,
        U: Into<u32> + Into<u8> + Copy,
    {
        check_component(DateComponent::Year, y)?;
        check_component(DateComponent::Month, m)?;
        Ok(PartialDate::Month((*y).into(), (*m).into()))
    }
    /**
     * Constructs a new `PartialDate` with a year, month and day precision
     * (YYYYMMDD)
     */
    pub fn from_ymd<T, U>(y: &T, m: &U, d: &U) -> Result<PartialDate>
    where
        T: Into<u32> + Into<u16> + Copy,
        U: Into<u32> + Into<u8> + Copy,
    {
        check_component(DateComponent::Year, y)?;
        check_component(DateComponent::Month, m)?;
        check_component(DateComponent::Day, d)?;
        Ok(PartialDate::Day((*y).into(), (*m).into(), (*d).into()))
    }
}

impl PartialTime {
    /**
     * Constructs a new `PartialTime` with hour precision
     * (HH)
     */
    pub fn from_h<T>(hour: &T) -> Result<PartialTime>
    where
        T: Into<u32> + Into<u8> + Copy,
    {
        check_component(DateComponent::Hour, hour)?;
        Ok(PartialTime::Hour((*hour).into()))
    }

    /**
     * Constructs a new `PartialTime` with hour and minute precision
     * (HHMM)
     */
    pub fn from_hm<T>(hour: &T, minute: &T) -> Result<PartialTime>
    where
        T: Into<u32> + Into<u8> + Copy,
    {
        check_component(DateComponent::Hour, hour)?;
        check_component(DateComponent::Minute, minute)?;
        Ok(PartialTime::Minute((*hour).into(), (*minute).into()))
    }

    /**
     * Constructs a new `PartialTime` with hour, minute and second precision
     * (HHMMSS)
     */
    pub fn from_hms<T>(hour: &T, minute: &T, second: &T) -> Result<PartialTime>
    where
        T: Into<u32> + Into<u8> + Copy,
    {
        check_component(DateComponent::Hour, hour)?;
        check_component(DateComponent::Minute, minute)?;
        check_component(DateComponent::Second, second)?;
        Ok(PartialTime::Second(
            (*hour).into(),
            (*minute).into(),
            (*second).into(),
        ))
    }

    /**
     * Constructs a new `PartialTime` with hour, minute, second and second fraction
     * (HHMMSS.FFFFFF) precision.
     * `frac_precision` (1-6) ... TODO
     */
    pub fn from_hmsf<T, U>(
        hour: &T,
        minute: &T,
        second: &T,
        fraction: &U,
        frac_precision: &T,
    ) -> Result<PartialTime>
    where
        T: Into<u32> + Into<u8> + Copy,
        U: Into<u32> + Copy,
    {
        let fp_copy: u32 = (*frac_precision).into();
        if !(1..=6).contains(&fp_copy) {
            return FractionPrecisionRange { value: fp_copy }.fail();
        }

        let fr_copy: u32 = (*fraction).into();
        if u32::pow(10, fp_copy) < fr_copy {
            return FractionPrecisionMismatch {
                fraction: fr_copy,
                precision: fp_copy,
            }
            .fail();
        }

        check_component(DateComponent::Hour, hour)?;
        check_component(DateComponent::Minute, minute)?;
        check_component(DateComponent::Second, second)?;
        let f: u32 = (*fraction).into() * u32::pow(10, 6 - fp_copy);
        check_component(DateComponent::Fraction, &f)?;
        Ok(PartialTime::Fraction(
            (*hour).into(),
            (*minute).into(),
            (*second).into(),
            fr_copy,
            (*frac_precision).into(),
        ))
    }
}

/*
impl PartialDateTime {
     /**
     * Constructs a new `PartialDate` with year precision
     * (YYYY)
     */
    pub fn from_y<T>(y: &T, offset: FixedOffset) -> Result<PartialDateTime>
    where
        T: Into<u32> + Into<u16> + Copy,
    {
        check_component(DateComponent::Year, y)?;
        Ok(PartialDate::Year((*y).into()))
    }

}*/

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_date() {
        assert_eq!(
            PartialDate::from_ymd(&1944u16, &2, &29).unwrap(),
            PartialDate::Day(1944, 2, 29)
        );
        assert_eq!(
            PartialDate::from_ym(&1944u16, &2).unwrap(),
            PartialDate::Month(1944, 2)
        );
        assert_eq!(
            PartialDate::from_y(&1944u16).unwrap(),
            PartialDate::Year(1944)
        );

        assert_eq!(
            PartialDate::from_ymd(&1944u16, &2, &29)
                .unwrap()
                .is_precise(),
            true
        );
        assert_eq!(
            PartialDate::from_ym(&1944u16, &2).unwrap().is_precise(),
            false
        );
        assert_eq!(PartialDate::from_y(&1944u16).unwrap().is_precise(), false);
        assert_eq!(
            PartialDate::from_ymd(&1944u16, &2, &29)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveDate::from_ymd(1944, 2, 29)
        );
        assert_eq!(
            PartialDate::from_ymd(&1944u16, &2, &29)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveDate::from_ymd(1944, 2, 29)
        );

        assert_eq!(
            PartialDate::from_y(&1944u16).unwrap().earliest().unwrap(),
            NaiveDate::from_ymd(1944, 1, 1)
        );
        // detects leap year
        assert_eq!(
            PartialDate::from_ym(&1944u16, &2)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveDate::from_ymd(1944, 2, 29)
        );
        assert_eq!(
            PartialDate::from_ym(&1945u16, &2)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveDate::from_ymd(1945, 2, 28)
        );
    }

    #[test]
    fn test_partial_time() {
        assert_eq!(
            PartialTime::from_hmsf(&9, &1, &1, &123456u32, &6).unwrap(),
            PartialTime::Fraction(9, 1, 1, 123456, 6)
        );
        assert_eq!(
            PartialTime::from_hmsf(&9, &1, &1, &1u32, &6).unwrap(),
            PartialTime::Fraction(9, 1, 1, 1, 6)
        );
        assert_eq!(
            PartialTime::from_hms(&9, &0, &0).unwrap(),
            PartialTime::Second(9, 0, 0)
        );
        assert_eq!(
            PartialTime::from_hm(&23, &59).unwrap(),
            PartialTime::Minute(23, 59)
        );
        assert_eq!(PartialTime::from_h(&1).unwrap(), PartialTime::Hour(1));

        assert_eq!(
            PartialTime::from_hmsf(&9, &1, &1, &123u32, &3)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 123_000)
        );
        assert_eq!(
            PartialTime::from_hmsf(&9, &1, &1, &123u32, &3)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 123_999)
        );

        assert_eq!(
            PartialTime::from_hmsf(&9, &1, &1, &1u32, &1)
                .unwrap()
                .earliest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 100_000)
        );
        assert_eq!(
            PartialTime::from_hmsf(&9, &1, &1, &1u32, &1)
                .unwrap()
                .latest()
                .unwrap(),
            NaiveTime::from_hms_micro(9, 1, 1, 199_999)
        );

        assert_eq!(
            PartialTime::from_hmsf(&9, &1, &1, &12345u32, &5)
                .unwrap()
                .is_precise(),
            false
        );

        assert_eq!(
            PartialTime::from_hmsf(&9, &1, &1, &123456u32, &6)
                .unwrap()
                .is_precise(),
            true
        );

        assert!(matches!(
            PartialTime::from_hmsf(&9, &1, &1, &1u32, &7),
            Err(Error::FractionPrecisionRange { value: 7, .. })
        ));

        assert!(matches!(
            PartialTime::from_hmsf(&9, &1, &1, &123456u32, &3),
            Err(Error::FractionPrecisionMismatch {
                fraction: 123456,
                precision: 3,
                ..
            })
        ));
    }
}
