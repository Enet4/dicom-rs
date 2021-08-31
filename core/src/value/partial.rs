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
 * where some components may be missing.
 */
#[derive(Debug, PartialEq)]
pub enum PartialDate {
    Year(u16),
    Month(u16, u8),
    Day(u16, u8, u8),
}

/**
 * Represents a Dicom Time value with a partial precision,
 * where some components may be missing.
 */
#[derive(Debug, PartialEq)]
pub enum PartialTime {
    Hour(u8),
    Minute(u8, u8),
    Second(u8, u8, u8),
    Fraction(u8, u8, u8, u32),
}

/**
 * Represents a Dicom DateTime value with a partial precision,
 * where some components may be missing.
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
    for<'a> &'a T: Into<u32>,
{
    let range = match component {
        DateComponent::Year => 0..=9_999,
        DateComponent::Month => 1..=12,
        DateComponent::Day => 1..=31,
        DateComponent::Hour => 0..=23,
        DateComponent::Minute => 0..=59,
        DateComponent::Second => 0..=59,
        DateComponent::Fraction => 0..=1_999_999,
        DateComponent::UTCOffset => 0..=86_399,
    };

    let value: u32 = value.into();
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
     * Constructs a new `PartialDate` with a Year precision
     * (YYYY)
     */
    pub fn from_y<T>(y: &T) -> Result<PartialDate>
    where
        for<'a> &'a T: Into<u32> + Into<u16>,
    {
        check_component(DateComponent::Year, y)?;
        Ok(PartialDate::Year(y.into()))
    }
    /**
     * Constructs a new `PartialDate` with a Year and Month precision
     * (YYYYMM)
     */
    pub fn from_ym<T, U>(y: &T, m: &U) -> Result<PartialDate>
    where
        for<'a> &'a T: Into<u32> + Into<u16>,
        for<'a> &'a U: Into<u32> + Into<u8>,
    {
        check_component(DateComponent::Year, y)?;
        check_component(DateComponent::Month, m)?;
        Ok(PartialDate::Month(y.into(), m.into()))
    }
    /**
     * Constructs a new `PartialDate` with a Year,Month and Day precision
     * (YYYYMMDD)
     */
    pub fn from_ymd<T, U>(y: &T, m: &U, d: &U) -> Result<PartialDate>
    where
        for<'a> &'a T: Into<u32> + Into<u16>,
        for<'a> &'a U: Into<u32> + Into<u8>,
    {
        check_component(DateComponent::Year, y)?;
        check_component(DateComponent::Month, m)?;
        check_component(DateComponent::Day, d)?;
        Ok(PartialDate::Day(y.into(), m.into(), d.into()))
    }
}

impl PartialTime {
    pub fn from_h<T>(h: &T) -> Result<PartialTime>
    where
        for<'a> &'a T: Into<u32> + Into<u8>,
    {
        check_component(DateComponent::Hour, h)?;
        Ok(PartialTime::Hour(h.into()))
    }

    pub fn from_hm<T>(h: &T, m: &T) -> Result<PartialTime>
    where
        for<'a> &'a T: Into<u32> + Into<u8>,
    {
        check_component(DateComponent::Hour, h)?;
        check_component(DateComponent::Minute, m)?;
        Ok(PartialTime::Minute(h.into(), m.into()))
    }

    pub fn from_hms<T>(h: &T, m: &T, s: &T) -> Result<PartialTime>
    where
        for<'a> &'a T: Into<u32> + Into<u8>,
    {
        check_component(DateComponent::Hour, h)?;
        check_component(DateComponent::Minute, m)?;
        check_component(DateComponent::Second, s)?;
        Ok(PartialTime::Second(h.into(), m.into(), s.into()))
    }

    pub fn from_hmsf<T, U>(h: &T, m: &T, s: &T, f: &U) -> Result<PartialTime>
    where
        for<'a> &'a T: Into<u32> + Into<u8>,
        for<'a> &'a U: Into<u32>,
    {
        check_component(DateComponent::Hour, h)?;
        check_component(DateComponent::Minute, m)?;
        check_component(DateComponent::Second, s)?;
        check_component(DateComponent::Fraction, f)?;
        Ok(PartialTime::Fraction(
            h.into(),
            m.into(),
            s.into(),
            f.into(),
        ))
    }
}

/**
 * This trait is implemented by partial precision
 * Date, Time and DateTime structures.
 * Trait method returns the last fully precise component of the value.
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
     * If structure contain invalid combination of date/time components, it fails.
     */
    fn earliest(&self) -> Result<T>;

    /**
     * Returns the latest possible value from a partial precision structure.
     * If structure contain invalid combination of date/time components, it fails.
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
     * Returns true, if partial precision structure has maximum possible accuracy.
     * For fraction of a second, ... TODO decide what to do, as RUST Chrono is more
     * precise than 6 digits of DICOM standard ...
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
        let (h, m, s, f) = match self {
            PartialTime::Hour(h) => (*h as u32, 0, 0, 0),
            PartialTime::Minute(h, m) => (*h as u32, *m as u32, 0, 0),
            PartialTime::Second(h, m, s) => (*h as u32, *m as u32, *s as u32, 0),
            PartialTime::Fraction(h, m, s, f) => (*h as u32, *m as u32, *s as u32, *f),
        };
        NaiveTime::from_hms_micro_opt(h, m, s, f).context(InvalidTime)
    }
    fn latest(&self) -> Result<NaiveTime> {
        let (h, m, s, f) = match self {
            PartialTime::Hour(h) => (*h as u32, 59, 59, 999_999),
            PartialTime::Minute(h, m) => (*h as u32, *m as u32, 59, 999_999),
            PartialTime::Second(h, m, s) => (*h as u32, *m as u32, *s as u32, 999_999),
            PartialTime::Fraction(h, m, s, f) => (*h as u32, *m as u32, *s as u32, *f),
        };
        NaiveTime::from_hms_micro_opt(h, m, s, f).context(InvalidTime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_precision() {
        let ymd = PartialDate::Day(1944, 2, 29);
        let ym = PartialDate::Month(1944, 2);
        let y = PartialDate::Year(1944);
        assert_eq!(ymd.is_precise(), true);
        assert_eq!(ym.is_precise(), false);
        assert_eq!(y.is_precise(), false);
        assert_eq!(ymd.earliest().unwrap(), NaiveDate::from_ymd(1944, 2, 29));
        assert_eq!(ymd.latest().unwrap(), NaiveDate::from_ymd(1944, 2, 29));
        assert_eq!(ym.earliest().unwrap(), NaiveDate::from_ymd(1944, 2, 1));
        // detects leap year
        assert_eq!(ym.latest().unwrap(), NaiveDate::from_ymd(1944, 2, 29));
        assert_eq!(y.latest().unwrap(), NaiveDate::from_ymd(1944, 12, 31));

        assert_eq!(
            PartialDate::Month(1945, 2).latest().unwrap(),
            NaiveDate::from_ymd(1945, 2, 28)
        );
    }

    #[test]
    fn test_time_precision() {
        let hmsf = PartialTime::Fraction(9, 1, 1, 1234567);
    }
}
