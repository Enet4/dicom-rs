//! Encoding of primitive values.
use crate::error::Result;
use dicom_core::chrono::{Datelike, FixedOffset, Timelike};
use dicom_core::value::{DateTime, NaiveDate, NaiveTime};
use std::io::Write;

pub fn encode_date<W>(mut to: W, date: NaiveDate) -> Result<()>
where
    W: Write,
{
    // YYYY(MM(DD)?)?
    write!(to, "{:04}{:02}{:02}", date.year(), date.month(), date.day())?;
    Ok(())
}

pub fn encode_time<W>(mut to: W, time: NaiveTime) -> Result<()>
where
    W: Write,
{
    // HH(MM(SS(.F{1,6})?)?)?

    let h = time.hour();
    let m = time.minute();
    let s = time.second();
    let f = time.nanosecond();

    match (h, m, s, f) {
        (h, 0, 0, 0) => write!(to, "{:02}", h,)?,
        (h, m, 0, 0) => write!(to, "{:02}{:02}", h, m)?,
        (h, m, s, 0) => write!(to, "{:02}{:02}{:02}", h, m, s)?,
        // 10ths of milliseconds
        (h, m, s, f) if f % 10_000_000 == 0 => {
            write!(to, "{:02}{:02}{:02}.{:02}", h, m, s, f / 10_000_000)?
        }
        // 100ths of microseconds
        (h, m, s, f) if f % 100_000 == 0 => {
            write!(to, "{:02}{:02}{:02}.{:04}", h, m, s, f / 100_000)?
        }
        _ => write!(to, "{:02}{:02}{:02}.{:06}", h, m, s, f / 1000)?,
    }
    Ok(())
}

pub fn encode_datetime<W>(mut to: W, dt: DateTime<FixedOffset>) -> Result<()>
where
    W: Write,
{
    encode_date(&mut to, dt.date().naive_utc())?;
    encode_time(&mut to, dt.time())?;
    let offset = *dt.offset();
    if offset != FixedOffset::east(0) {
        let offset_hm = offset.local_minus_utc() / 60;
        let offset_h = offset_hm / 60;
        let offset_m = offset_hm % 60;
        write!(to, "{:+03}{:02}", offset_h, offset_m)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use dicom_core::chrono::TimeZone;
    use dicom_core::value::NaiveDate;
    use std::str::from_utf8;

    #[test]
    fn test_encode_date() {
        let mut data = vec![];
        encode_date(&mut data, NaiveDate::from_ymd(1985, 12, 31)).unwrap();
        assert_eq!(&data, &*b"19851231");
    }

    #[test]
    fn test_encode_time() {
        let mut data = vec![];
        encode_time(&mut data, NaiveTime::from_hms_micro(23, 59, 48, 123456)).unwrap();
        assert_eq!(&data, &*b"235948.123456");

        let mut data = vec![];
        encode_time(&mut data, NaiveTime::from_hms_micro(12, 0, 30, 0)).unwrap();
        assert_eq!(&data, &*b"120030");

        let mut data = vec![];
        encode_time(&mut data, NaiveTime::from_hms_micro(9, 0, 0, 0)).unwrap();
        assert_eq!(&data, &*b"09");
    }

    #[test]
    fn test_encode_datetime() {
        let mut data = vec![];
        encode_datetime(
            &mut data,
            FixedOffset::east(0)
                .ymd(1985, 12, 31)
                .and_hms_micro(23, 59, 48, 123456),
        )
        .unwrap();
        assert_eq!(from_utf8(&data).unwrap(), "19851231235948.123456");

        let mut data = vec![];
        encode_datetime(
            &mut data,
            FixedOffset::east(3_600).ymd(2018, 12, 24).and_hms(4, 0, 0),
        )
        .unwrap();
        assert_eq!(from_utf8(&data).unwrap(), "2018122404+0100");
    }
}
