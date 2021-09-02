use std::io::Write;

/// A P-Data value writer.
///
/// This exposes an API to iteratively construct and send Data messages
/// to another node.
/// Using this as a [standard writer](std::io::Write)
/// will automatically split the incoming bytes
/// into separate PDUs if they do not fit in a single one.
///
/// Use an association's `send_pdata` method
/// to create a new P-Data value writer.
#[must_use]
pub struct PDataWriter<W: Write> {
    buffer: Vec<u8>,
    stream: W,
    presentation_context_id: u8,
    max_data_length: u32,
}

impl<W> PDataWriter<W>
where
    W: Write,
{
    /// Construct a new P-Data value writer.
    pub(crate) fn new(stream: W, presentation_context_id: u8, max_pdu_length: u32) -> Self {
        let max_data_length = calculate_max_data_len_single(max_pdu_length);
        PDataWriter {
            stream,
            presentation_context_id,
            max_data_length,
            buffer: Vec::with_capacity(max_data_length as usize),
        }
    }

    /// Send the header of a single P-Data PDU,
    /// containing a single data fragment.
    fn send_pdata_header(&mut self, data_len: u32, is_last: bool) -> std::io::Result<()> {
        let mut message_header = 0x00;
        if is_last {
            message_header |= 0x02;
        }

        let pdu_len_bytes = (data_len + 2 + 4).to_be_bytes();
        let data_len_bytes = (data_len + 2).to_be_bytes();
        let header = [
            // PDU-type + reserved byte
            0x04,
            0x00,
            // full PDU length
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
            message_header,
        ];

        self.stream.write_all(&header)
    }

    fn finish(&mut self) -> std::io::Result<()> {
        self.dispatch_excess_data()?;
        if !self.buffer.is_empty() {
            // send last PDU
            self.send_pdata_header(self.buffer.len() as u32, true)?;
            self.stream.write_all(&self.buffer[..])?;
            self.buffer.clear();
        }
        Ok(())
    }

    fn dispatch_excess_data(&mut self) -> std::io::Result<()> {
        while self.buffer.len() > self.max_data_length as usize {
            // send PDU now
            self.send_pdata_header(self.max_data_length, false)?;
            let data = &self.buffer[0..self.max_data_length as usize];
            self.stream.write_all(data)?;

            // shift the remaining contents to the beginning of the buffer
            let (p1, p2) = (&mut self.buffer[..]).split_at_mut(self.max_data_length as usize);
            for (e1, e2) in std::iter::Iterator::zip(p1.iter_mut(), p2.iter()) {
                *e1 = *e2;
            }
            self.buffer
                .truncate(self.buffer.len() - self.max_data_length as usize);
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

impl<W> Drop for PDataWriter<W>
where
    W: Write,
{
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

/// Determine the maximum length of actual data
/// when encapsulated in a PDU with the given length property.
/// Does not account for the first 2 bytes (type + reserved).
#[inline]
fn calculate_max_data_len_single(pdu_len: u32) -> u32 {
    pdu_len
    // data length
    - 4
    // control header
    - 2
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::pdu::PDataValueType;
    use crate::pdu::Pdu;
    use crate::pdu::{read_pdu, reader::MINIMUM_PDU_SIZE};

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

        let my_data: Vec<_> = (0..6000).map(|x| x as u8).collect();
        assert_eq!(my_data.len(), 6000);

        let mut buf = Vec::new();
        {
            let mut writer = PDataWriter::new(&mut buf, presentation_context_id, MINIMUM_PDU_SIZE);
            writer.write_all(&my_data).unwrap();
            writer.finish().unwrap();
        }

        let mut cursor = &buf[..];
        let same_pdu_1 = read_pdu(&mut cursor, MINIMUM_PDU_SIZE, true).unwrap();
        let same_pdu_2 = read_pdu(&mut cursor, MINIMUM_PDU_SIZE, true).unwrap();

        // concatenate data chunks, compare with all data

        match (same_pdu_1, same_pdu_2) {
            (Pdu::PData { data: data_1 }, Pdu::PData { data: data_2 }) => {
                let data_1 = &data_1[0];
                let data_2 = &data_2[0];

                // check that these two PDUs are consistent
                assert_eq!(data_1.value_type, PDataValueType::Data);
                assert_eq!(data_2.value_type, PDataValueType::Data);
                assert_eq!(data_1.presentation_context_id, presentation_context_id);
                assert_eq!(data_2.presentation_context_id, presentation_context_id);
                assert_eq!(data_1.data.len() + data_2.data.len(), 6000);

                let data_1 = &data_1.data;
                let data_2 = &data_2.data;

                let mut all_data: Vec<u8> = Vec::new();
                all_data.extend(data_1);
                all_data.extend(data_2);
                assert_eq!(all_data, my_data);
            }
            x => panic!("Expected two PDatas, got {:?}", x),
        }

        assert_eq!(cursor.len(), 0);
    }
}
