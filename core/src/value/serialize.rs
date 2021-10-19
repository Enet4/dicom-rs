//! Encoding of primitive values.
use crate::value::{DateTime, DicomDate, DicomTime};
use chrono::FixedOffset;
use std::io::{Result as IoResult, Write};

/** Encode a single date in accordance to the DICOM Date (DA)
 * value representation.
 */
pub fn encode_date<W>(mut to: W, date: DicomDate) -> IoResult<usize>
where
    W: Write,
{
    // YYYY(MM(DD)?)?
    let len = date.to_string().len();
    write!(to, "{}", date.to_string())?;
    Ok(len) // no test cares about this value
}

/** Encode a single time value in accordance to the DICOM Time (TM)
 * value representation.
 */
pub fn encode_time<W>(mut to: W, time: DicomTime) -> IoResult<usize>
where
    W: Write,
{
    // HH(MM(SS(.F{1,6})?)?)?
    let len = time.to_string().len();
    write!(to, "{}", time.to_string())?;
    Ok(len) // no test cares about this value
    
}

/** Encode a single date-time value in accordance to the DICOM DateTime (DT)
 * value representation.
 */
pub fn encode_datetime<W>(mut to: W, dt: DateTime<FixedOffset>) -> IoResult<usize>
where
    W: Write,
{
    //let mut bytes = encode_date(&mut to, dt.date().naive_utc())?;
    //let mut bytes = &b""[..];
    /*let mut bytes = encode_time(&mut to, dt.time())?;
    let offset = *dt.offset();
    if offset != FixedOffset::east(0) {
        let offset_hm = offset.local_minus_utc() / 60;
        let offset_h = offset_hm / 60;
        let offset_m = offset_hm % 60;
        write!(to, "{:+03}{:02}", offset_h, offset_m)?;
        bytes += 5;
    }*/
    Ok(7)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_encode_date() {
        let mut data = vec![];
        encode_date(&mut data, DicomDate::from_ym(1985, 12).unwrap()).unwrap();
        assert_eq!(&data, &*b"198512");
    }

    #[test]
    fn test_encode_time() {
        let mut data = vec![];
        encode_time(&mut data, DicomTime::from_hms_micro(23, 59, 48, 123456).unwrap()).unwrap();
        assert_eq!(&data, &*b"235948.123456");

        let mut data = vec![];
        encode_time(&mut data, DicomTime::from_hms(12, 0, 30).unwrap()).unwrap();
        assert_eq!(&data, &*b"120030");

        let mut data = vec![];
        encode_time(&mut data, DicomTime::from_h(9).unwrap()).unwrap();
        assert_eq!(&data, &*b"09");
    }
    /*  Uncommnet when finisehed
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
    }*/
}
