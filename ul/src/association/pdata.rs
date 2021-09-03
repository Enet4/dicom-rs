use std::io::Write;

use crate::pdu::reader::PDU_HEADER_SIZE;

/// A P-Data value writer.
///
/// This exposes an API to iteratively construct and send Data messages
/// to another node.
/// Using this as a [standard writer](std::io::Write)
/// will automatically split the incoming bytes
/// into separate PDUs if they do not fit in a single one.
///
/// # Example
///
/// Use an association's `send_pdata` method
/// to create a new P-Data value writer.
///
/// ```no_run
/// # use std::io::Write;
/// # use dicom_ul::association::{ClientAssociationOptions, PDataWriter};
/// # use dicom_ul::pdu::{Pdu, PDataValue, PDataValueType};
/// # fn command_data() -> Vec<u8> { unimplemented!() }
/// # fn dicom_data() -> &'static [u8] { unimplemented!() }
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut association = ClientAssociationOptions::new()
///    .establish("129.168.0.5:104")?;
///
/// let presentation_context_id = association.presentation_contexts()[0].id;
///
/// // send a command first
/// association.send(&Pdu::PData {
///     data: vec![PDataValue {
///     presentation_context_id,
///     value_type: PDataValueType::Command,
///         is_last: true,
///         data: command_data(),
///     }],
/// });
///
/// // then send a DICOM object which may be split into multiple PDUs
/// let mut pdata = association.send_pdata(presentation_context_id);
/// pdata.write_all(dicom_data())?;
/// pdata.finish()?;
///
/// let pdu_ac = association.receive()?;
/// # Ok(())
/// # }
#[must_use]
pub struct PDataWriter<W: Write> {
    buffer: Vec<u8>,
    stream: W,
    presentation_context_id: u8,
    max_data_len: u32,
}

impl<W> PDataWriter<W>
where
    W: Write,
{
    /// Construct a new P-Data value writer.
    ///
    /// `max_pdu_length` is the maximum value of the PDU-length property.
    pub(crate) fn new(stream: W, presentation_context_id: u8, max_pdu_length: u32) -> Self {
        let max_data_length = calculate_max_data_len_single(max_pdu_length);
        PDataWriter {
            stream,
            presentation_context_id,
            max_data_len: max_data_length,
            buffer: Vec::with_capacity(max_data_length as usize),
        }
    }

    /// Declare to have finished sending P-Data fragments,
    /// thus emitting the last P-Data fragment PDU.
    ///
    /// This is also done automatically once the P-Data writer is dropped.
    pub fn finish(mut self) -> std::io::Result<()> {
        self.finish_impl()?;
        Ok(())
    }

    fn create_pdata_header(&self, data_len: u32, is_last: bool) -> [u8; 12] {
        let pdu_len = data_len + 4 + 2;
        let pdv_data_len = data_len + 2;
        let pdu_len_bytes = pdu_len.to_be_bytes();
        let data_len_bytes = pdv_data_len.to_be_bytes();
        [
            // PDU-type + reserved byte
            0x04,
            0x00,
            // full PDU length (minus PDU type and reserved byte)
            pdu_len_bytes[0],
            pdu_len_bytes[1],
            pdu_len_bytes[2],
            pdu_len_bytes[3],
            // presentation data length (data + 2 properties below)
            data_len_bytes[0],
            data_len_bytes[1],
            data_len_bytes[2],
            data_len_bytes[3],
            // presentation context id
            self.presentation_context_id,
            // message control header
            if is_last { 0x02 } else { 0x00 },
        ]
    }

    /// Send the header of a single P-Data PDU,
    /// containing a single data fragment.
    fn send_pdata_header(&mut self, data_len: u32, is_last: bool) -> std::io::Result<()> {
        self.stream
            .write_all(&self.create_pdata_header(data_len, is_last))
    }

    fn finish_impl(&mut self) -> std::io::Result<()> {
        self.dispatch_excess_data()?;
        if !self.buffer.is_empty() {
            // send last PDU
            self.send_pdata_header(self.buffer.len() as u32, true)?;
            self.stream.write_all(&self.buffer[..])?;
            // clear buffer so that subsequent calls to `finish_impl`
            // do not send any more PDUs
            self.buffer.clear();
        }
        Ok(())
    }

    /// Use the current state of the buffer to send new PDUs
    ///
    /// Pre-condition:
    /// buffer must have enough data for one full P-Data-tf PDU
    fn dispatch_pdus(&mut self) -> std::io::Result<()> {
        debug_assert!(self.buffer.len() >= self.max_data_len as usize);
        // send PDU now

        let mut acc_len = 0;
        for chunk in self.buffer.chunks(self.max_data_len as usize) {
            if chunk.len() < self.max_data_len as usize {
                break;
            }
            let header = self.create_pdata_header(self.max_data_len, false);
            self.stream.write_all(&header)?;
            self.stream.write_all(chunk)?;
            acc_len += chunk.len();
        }

        // shift the remaining contents to the beginning of the buffer
        // (guaranteed to be shorter than `self.max_data_len`)
        let (p1, p2) = (&mut self.buffer[..]).split_at_mut(acc_len);
        for (e1, e2) in std::iter::Iterator::zip(p1.iter_mut(), p2.iter()) {
            *e1 = *e2;
        }
        self.buffer.truncate(self.buffer.len() - acc_len as usize);

        Ok(())
    }

    fn dispatch_excess_data(&mut self) -> std::io::Result<()> {
        while self.buffer.len() > self.max_data_len as usize {
            self.dispatch_pdus()?;
        }
        Ok(())
    }
}

impl<W> Write for PDataWriter<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend(buf);
        self.dispatch_excess_data()?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // do nothing
        Ok(())
    }
}

/// With the P-Data writer dropped,
/// this `Drop` implementation
/// will construct and emit the last P-Data fragment PDU
/// if there is any data left to send.
impl<W> Drop for PDataWriter<W>
where
    W: Write,
{
    fn drop(&mut self) {
        let _ = self.finish_impl();
    }
}

/// Determine the maximum length of actual PDV data
/// when encapsulated in a PDU with the given length property.
/// Does not account for the first 2 bytes (type + reserved).
#[inline]
fn calculate_max_data_len_single(pdu_len: u32) -> u32 {
    // data length: 4 bytes
    // control header: 2 bytes
    pdu_len - PDU_HEADER_SIZE
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::pdu::reader::{read_pdu, MINIMUM_PDU_SIZE, PDU_HEADER_SIZE};
    use crate::pdu::PDataValueType;
    use crate::pdu::Pdu;

    use super::PDataWriter;

    #[test]
    fn test_write_pdata_and_finish() {
        let presentation_context_id = 12;

        let mut buf = Vec::new();
        {
            let mut writer = PDataWriter::new(&mut buf, presentation_context_id, MINIMUM_PDU_SIZE);
            writer.write_all(&(0..64).collect::<Vec<u8>>()).unwrap();
            writer.finish().unwrap();
        }

        let mut cursor = &buf[..];
        let same_pdu = read_pdu(&mut cursor, MINIMUM_PDU_SIZE, true).unwrap();

        // concatenate data chunks, compare with all data

        match same_pdu {
            Pdu::PData { data: data_1 } => {
                let data_1 = &data_1[0];

                // check that this PDU is consistent
                assert_eq!(data_1.value_type, PDataValueType::Data);
                assert_eq!(data_1.presentation_context_id, presentation_context_id);
                assert_eq!(data_1.data.len(), 64);
                assert_eq!(data_1.data, (0..64).collect::<Vec<u8>>());
            }
            pdu => panic!("Expected PData, got {:?}", pdu),
        }

        assert_eq!(cursor.len(), 0);
    }

    #[test]
    fn test_write_large_pdata_and_finish() {
        let presentation_context_id = 32;

        let my_data: Vec<_> = (0..9000).map(|x: u32| x as u8).collect();

        let mut buf = Vec::new();
        {
            let mut writer = PDataWriter::new(&mut buf, presentation_context_id, MINIMUM_PDU_SIZE);
            writer.write_all(&my_data).unwrap();
            writer.finish().unwrap();
        }

        let mut cursor = &buf[..];
        let pdu_1 = read_pdu(&mut cursor, MINIMUM_PDU_SIZE, true).unwrap();
        let pdu_2 = read_pdu(&mut cursor, MINIMUM_PDU_SIZE, true).unwrap();
        let pdu_3 = read_pdu(&mut cursor, MINIMUM_PDU_SIZE, true).unwrap();

        // concatenate data chunks, compare with all data

        match (pdu_1, pdu_2, pdu_3) {
            (
                Pdu::PData { data: data_1 },
                Pdu::PData { data: data_2 },
                Pdu::PData { data: data_3 },
            ) => {
                assert_eq!(data_1.len(), 1);
                let data_1 = &data_1[0];
                assert_eq!(data_2.len(), 1);
                let data_2 = &data_2[0];
                assert_eq!(data_3.len(), 1);
                let data_3 = &data_3[0];

                // check that these two PDUs are consistent
                assert_eq!(data_1.value_type, PDataValueType::Data);
                assert_eq!(data_2.value_type, PDataValueType::Data);
                assert_eq!(data_1.presentation_context_id, presentation_context_id);
                assert_eq!(data_2.presentation_context_id, presentation_context_id);

                // check expected lengths
                assert_eq!(
                    data_1.data.len(),
                    (MINIMUM_PDU_SIZE - PDU_HEADER_SIZE) as usize
                );
                assert_eq!(
                    data_2.data.len(),
                    (MINIMUM_PDU_SIZE - PDU_HEADER_SIZE) as usize
                );
                assert_eq!(data_3.data.len(), 820);

                // check data consistency
                assert_eq!(
                    &data_1.data[..],
                    (0..MINIMUM_PDU_SIZE - PDU_HEADER_SIZE).map(|x| x as u8).collect::<Vec<_>>()
                );
                assert_eq!(
                    data_1.data.len() + data_2.data.len() + data_3.data.len(),
                    9000
                );

                let data_1 = &data_1.data;
                let data_2 = &data_2.data;
                let data_3 = &data_3.data;

                let mut all_data: Vec<u8> = Vec::new();
                all_data.extend(data_1);
                all_data.extend(data_2);
                all_data.extend(data_3);
                assert_eq!(all_data, my_data);
            }
            x => panic!("Expected 3 PDatas, got {:?}", x),
        }

        assert_eq!(cursor.len(), 0);
    }
}
