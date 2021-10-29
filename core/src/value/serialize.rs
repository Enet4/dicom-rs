//! Encoding of primitive values.
use crate::value::{DicomDate, DicomDateTime, DicomTime};
use std::io::{Result as IoResult, Write};

/** Encode a single date in accordance to the DICOM Date (DA)
 * value representation.
 */
pub fn encode_date<W>(mut to: W, date: DicomDate) -> IoResult<usize>
where
    W: Write,
{
    // YYYY(MM(DD)?)?
    let len = date.to_encoded().len();
    write!(to, "{}", date.to_encoded())?;
    Ok(len) 
}

/** Encode a single time value in accordance to the DICOM Time (TM)
 * value representation.
 */
pub fn encode_time<W>(mut to: W, time: DicomTime) -> IoResult<usize>
where
    W: Write,
{
    // HH(MM(SS(.F{1,6})?)?)?
    let len = time.to_encoded().len();
    write!(to, "{}", time.to_encoded())?;
    Ok(len) 
}

/** Encode a single date-time value in accordance to the DICOM DateTime (DT)
 * value representation.
 */
pub fn encode_datetime<W>(mut to: W, dt: DicomDateTime) -> IoResult<usize>
where
    W: Write,
{
    let len = dt.to_string().len();
    write!(to, "{}", dt.to_encoded())?;
    Ok(len) 
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::FixedOffset;
    use std::str::from_utf8;

    #[test]
    fn test_encode_date() {
        let mut data = vec![];
        encode_date(&mut data, DicomDate::from_ym(1985, 12).unwrap()).unwrap();
        assert_eq!(&data, &*b"198512");
    }

    #[test]
    fn test_encode_time() {
        let mut data = vec![];
        encode_time(
            &mut data,
            DicomTime::from_hms_micro(23, 59, 48, 123456).unwrap(),
        )
        .unwrap();
        assert_eq!(&data, &*b"235948.123456");

        let mut data = vec![];
        encode_time(&mut data, DicomTime::from_hms(12, 0, 30).unwrap()).unwrap();
        assert_eq!(&data, &*b"120030");

        let mut data = vec![];
        encode_time(&mut data, DicomTime::from_h(9).unwrap()).unwrap();
        assert_eq!(&data, &*b"09");
    }

    #[test]
    fn test_encode_datetime() {
        let mut data = vec![];
        let offset = FixedOffset::east(0);
        encode_datetime(
            &mut data,
            DicomDateTime::from_dicom_date_and_time(
                DicomDate::from_ymd(1985, 12, 31).unwrap(),
                DicomTime::from_hms_micro(23, 59, 48, 123_456).unwrap(),
                offset,
            )
            .unwrap(),
        )
        .unwrap();
        // even zero offset gets encoded into string value
        assert_eq!(from_utf8(&data).unwrap(), "19851231235948.123456+0000");

        let mut data = vec![];
        let offset = FixedOffset::east(3600);
        encode_datetime(
            &mut data,
            DicomDateTime::from_dicom_date_and_time(
                DicomDate::from_ymd(2018, 12, 24).unwrap(),
                DicomTime::from_h(4).unwrap(),
                offset,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(from_utf8(&data).unwrap(), "2018122404+0100");
    }
}
