use std::{
    collections::VecDeque, io::{BufRead, BufReader, Cursor, Read, Write}
};

use bytes::{Buf, BytesMut};
use snafu::ResultExt;
#[cfg(feature = "async")]
use tokio::io::ReadBuf;
#[cfg(feature = "async")]
use std::{pin::Pin, task::{Context, Poll}};
use tracing::warn;

use crate::{pdu::{ReadPduSnafu, PDU_HEADER_SIZE}, read_pdu, Pdu};

#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// Set up the P-Data PDU header for sending.
fn setup_pdata_header(buffer: &mut Vec<u8>, is_last: bool) {
    let data_len = (buffer.len() - 12) as u32;

    // full PDU length (minus PDU type and reserved byte)
    let pdu_len = data_len + 4 + 2;
    let pdu_len_bytes = pdu_len.to_be_bytes();

    buffer[2] = pdu_len_bytes[0];
    buffer[3] = pdu_len_bytes[1];
    buffer[4] = pdu_len_bytes[2];
    buffer[5] = pdu_len_bytes[3];

    // presentation data length (data + 2 properties below)
    let pdv_data_len = data_len + 2;
    let data_len_bytes = pdv_data_len.to_be_bytes();

    buffer[6] = data_len_bytes[0];
    buffer[7] = data_len_bytes[1];
    buffer[8] = data_len_bytes[2];
    buffer[9] = data_len_bytes[3];

    // message control header
    buffer[11] = if is_last { 0x02 } else { 0x00 };
}


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
        let mut buffer = Vec::with_capacity((max_data_length + PDU_HEADER_SIZE) as usize);
        // initial buffer set up
        buffer.extend([
            // PDU-type + reserved byte
            0x04,
            0x00,
            // full PDU length, unknown at this point
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            // presentation data length, unknown at this point
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            // presentation context id
            presentation_context_id,
            // message control header, unknown at this point
            0xFF,
        ]);

        PDataWriter {
            stream,
            max_data_len: max_data_length,
            buffer,
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

    fn finish_impl(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_empty() {
            // send last PDU
            setup_pdata_header(&mut self.buffer, true);
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
    /// buffer must have enough data for one P-Data-tf PDU
    fn dispatch_pdu(&mut self) -> std::io::Result<()> {
        debug_assert!(self.buffer.len() >= 12);
        // send PDU now
        setup_pdata_header(&mut self.buffer, false);
        self.stream.write_all(&self.buffer)?;

        // back to just the header
        self.buffer.truncate(12);

        Ok(())
    }
}

impl<W> Write for PDataWriter<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let total_len = self.max_data_len as usize + 12;
        if self.buffer.len() + buf.len() <= total_len {
            // accumulate into buffer, do nothing
            self.buffer.extend(buf);
            Ok(buf.len())
        } else {
            // fill in the rest of the buffer, send PDU,
            // and leave out the rest for subsequent writes
            let buf = &buf[..total_len - self.buffer.len()];
            self.buffer.extend(buf);
            debug_assert_eq!(self.buffer.len(), total_len);
            self.dispatch_pdu()?;
            Ok(buf.len())
        }
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

/// A P-Data async value writer.
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
#[cfg(feature = "async")]
#[must_use]
pub struct AsyncPDataWriter<W: AsyncWrite + Unpin> {
    buffer: Vec<u8>,
    stream: W,
    max_data_len: u32,
}

#[cfg(feature = "async")]
impl<W> AsyncPDataWriter<W>
where
    W: AsyncWrite + Unpin,
{
    /// Construct a new P-Data value writer.
    ///
    /// `max_pdu_length` is the maximum value of the PDU-length property.
    pub(crate) fn new(stream: W, presentation_context_id: u8, max_pdu_length: u32) -> Self {
        let max_data_length = calculate_max_data_len_single(max_pdu_length);
        let mut buffer = Vec::with_capacity((max_data_length + PDU_HEADER_SIZE) as usize);
        // initial buffer set up
        buffer.extend([
            // PDU-type + reserved byte
            0x04,
            0x00,
            // full PDU length, unknown at this point
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            // presentation data length, unknown at this point
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            // presentation context id
            presentation_context_id,
            // message control header, unknown at this point
            0xFF,
        ]);

        AsyncPDataWriter {
            stream,
            max_data_len: max_data_length,
            buffer,
        }
    }

    /// Declare to have finished sending P-Data fragments,
    /// thus emitting the last P-Data fragment PDU.
    ///
    /// This is also done automatically once the P-Data writer is dropped.
    pub async fn finish(mut self) -> std::io::Result<()> {
        self.finish_impl().await?;
        Ok(())
    }

    async fn finish_impl(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_empty() {
            // send last PDU
            setup_pdata_header(&mut self.buffer, true);
            self.stream.write_all(&self.buffer[..]).await?;
            // clear buffer so that subsequent calls to `finish_impl`
            // do not send any more PDUs
            self.buffer.clear();
        }
        Ok(())
    }
}

#[cfg(feature = "async")]
impl<W> AsyncWrite for AsyncPDataWriter<W>
where
    W: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::result::Result<usize, std::io::Error>>{
        let total_len = self.max_data_len as usize + 12;
        if self.buffer.len() + buf.len() <= total_len {
            // accumulate into buffer, do nothing
            self.buffer.extend(buf);
            Poll::Ready(Ok(buf.len()))
        } else {
            // fill in the rest of the buffer, send PDU,
            // and leave out the rest for subsequent writes
            let buf = &buf[..total_len - self.buffer.len()];
            self.buffer.extend(buf);
            debug_assert_eq!(self.buffer.len(), total_len);
            setup_pdata_header(&mut self.buffer, false);
            // Avoid multiple mutable borrows, take self.buffer then return it after poll_write
            let data = std::mem::take(&mut self.buffer);
            let data_len = data.len();
            let mut position = 0;
            while position < data_len {
                let res = Pin::new(&mut self.stream)
                    .poll_write(cx, &data[position..]);
                match res {
                    Poll::Ready(Ok(n)) => {
                        // Update position
                        position += n;
                    },
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending
                }
            }
            self.buffer = Vec::from(&data[..12]);
            return Poll::Ready(Ok(position))
        }

    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), std::io::Error>>{
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), std::io::Error>>{
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

/// With the P-Data writer dropped,
/// this `Drop` implementation
/// will construct and emit the last P-Data fragment PDU
/// if there is any data left to send.
#[cfg(feature = "async")]
impl<W> Drop for AsyncPDataWriter<W>
where
    W: AsyncWrite + Unpin,
{
    fn drop(&mut self) {
        tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async move {
                let _ = self.finish_impl().await;
            })
        })
    }
}

/// A P-Data value reader.
///
/// This exposes an API which provides a byte stream of data
/// by iteratively collecting Data messages from another node.
/// Using this as a [standard reader](std::io::Read)
/// will provide all incoming bytes,
/// even if they reside in separate PDUs,
/// until the last message is received.
///
/// # Example
///
/// Use an association's `receive_pdata` method
/// to create a new P-Data value reader.
///
/// ```no_run
/// # use std::io::Read;
/// # use dicom_ul::association::{ClientAssociationOptions, PDataReader};
/// # use dicom_ul::pdu::{Pdu, PDataValue, PDataValueType};
/// # fn command_data() -> Vec<u8> { unimplemented!() }
/// # fn dicom_data() -> &'static [u8] { unimplemented!() }
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let mut association = ClientAssociationOptions::new()
/// #    .establish("129.168.0.5:104")?;
///
/// // expecting a DICOM object which may be split into multiple PDUs,
/// let mut pdata = association.receive_pdata();
/// let all_pdata_bytes = {
///     let mut v = Vec::new();
///     pdata.read_to_end(&mut v)?;
///     v
/// };
/// # Ok(())
/// # }
#[must_use]
pub struct PDataReader<R> {
    buffer: VecDeque<u8>,
    stream: R,
    presentation_context_id: Option<u8>,
    max_data_length: u32,
    last_pdu: bool,
    read_buffer: BytesMut,
}

impl<R> PDataReader<R>
{
    pub fn new(stream: R, max_data_length: u32) -> Self {
        PDataReader {
            buffer: VecDeque::with_capacity(max_data_length as usize),
            stream,
            presentation_context_id: None,
            max_data_length,
            last_pdu: false,
            read_buffer: BytesMut::with_capacity(max_data_length as usize),
        }
    }

    /// Declare no intention to read more PDUs from the remote node.
    ///
    /// Attempting to read more bytes
    /// will only consume the inner buffer and not result in
    /// more PDUs being received.
    pub fn stop_receiving(&mut self) -> std::io::Result<()> {
        self.last_pdu = true;
        Ok(())
    }
}

impl<R> Read for PDataReader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.buffer.is_empty() {
            if self.last_pdu {
                // reached the end of PData stream
                return Ok(0);
            }

            let mut reader = BufReader::new(&mut self.stream);
            let msg = loop{
                let mut buf = Cursor::new(&self.read_buffer[..]);
                match read_pdu(&mut buf, self.max_data_length, false)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))? {
                    Some(pdu) => {
                       self.read_buffer.advance(buf.position() as usize);
                        break pdu
                    },
                    None => {
                        // Reset position
                        buf.set_position(0)
                    }
                }
                let recv = reader.fill_buf()?.to_vec();
                reader.consume(recv.len());
                self.read_buffer.extend_from_slice(&recv);
                if recv.len() == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other, "Connection closed by peer"
                    ));
                }
            };

            match msg {
                Pdu::PData { data } => {
                    for pdata_value in data {
                        self.presentation_context_id = match self.presentation_context_id {
                            None => Some(pdata_value.presentation_context_id),
                            Some(cid) if cid == pdata_value.presentation_context_id => Some(cid),
                            Some(cid) => {
                                warn!("Received PData value of presentation context {}, but should be {}", pdata_value.presentation_context_id, cid);
                                Some(cid)
                            }
                        };
                        self.buffer.extend(pdata_value.data);
                        self.last_pdu = pdata_value.is_last;
                    }
                }
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Unexpected PDU type",
                    ))
                }
            }
        }
        Read::read(&mut self.buffer, buf)
    }
}

#[cfg(feature = "async")]
impl<R> AsyncRead for PDataReader<R> where R: AsyncRead + Unpin {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>>{
        if self.buffer.is_empty(){
            if self.last_pdu {
                // reached the end of PData stream
                return Poll::Ready(Ok(()));
            }
            let mut read_buffer = BytesMut::with_capacity(self.max_data_length as usize);
            let msg = loop {
                let mut buf = Cursor::new(&read_buffer[..]);
                match read_pdu(&mut buf, self.max_data_length, false)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))? {
                    Some(pdu) => {
                        read_buffer.advance(buf.position() as usize);
                        break pdu
                    },
                    None => {
                        // Reset position
                        buf.set_position(0)
                    }
                }
                // Do the actual read from the socket
                let recv = Pin::new(&mut self.stream)
                    .poll_read(cx, &mut ReadBuf::new(&mut read_buffer));
                match recv {
                    Poll::Ready(Ok(())) => {
                        continue
                    },
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e))
                }
            };
            
            match msg {
                Pdu::PData { data } => {
                    for pdata_value in data {
                        self.presentation_context_id = match self.presentation_context_id {
                            None => Some(pdata_value.presentation_context_id),
                            Some(cid) if cid == pdata_value.presentation_context_id => Some(cid),
                            Some(cid) => {
                                warn!("Received PData value of presentation context {}, but should be {}", pdata_value.presentation_context_id, cid);
                                Some(cid)
                            }
                        };
                        self.buffer.extend(pdata_value.data);
                        self.last_pdu = pdata_value.is_last;
                    }
                }
                _ => {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Unexpected PDU type",
                    )))
                }
            }
        }
        // Naive implementation of `Read::read` for VecDeque
        while let Some(&byte) = self.buffer.front() {
            if buf.remaining() == 0 {
                return Poll::Ready(Ok(()));
            }
            buf.put_slice(&[byte]);
            self.buffer.pop_front();
        }
        Poll::Ready(Ok(()))
    }
}
/// Determine the maximum length of actual PDV data
/// when encapsulated in a PDU with the given length property.
/// Does not account for the first 2 bytes (type + reserved).
#[inline]
fn calculate_max_data_len_single(pdu_len: u32) -> u32 {
    // data length: 4 bytes
    // control header: 2 bytes
    pdu_len - 4 - 2
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io::{Read, Write};

    use crate::pdu::{read_pdu, MINIMUM_PDU_SIZE, PDU_HEADER_SIZE, Pdu};
    use crate::pdu::{PDataValue, PDataValueType};
    use crate::write_pdu;

    use super::{PDataReader, PDataWriter};

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

        match same_pdu.unwrap() {
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

        match (pdu_1.unwrap(), pdu_2.unwrap(), pdu_3.unwrap()) {
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
                    (0..MINIMUM_PDU_SIZE - PDU_HEADER_SIZE)
                        .map(|x| x as u8)
                        .collect::<Vec<_>>()
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

    #[test]
    fn test_read_large_pdata_and_finish() {
        let presentation_context_id = 32;

        let my_data: Vec<_> = (0..9000).map(|x: u32| x as u8).collect();
        let pdata_1 = vec![PDataValue {
            value_type: PDataValueType::Data,
            data: my_data[0..3000].to_owned(),
            presentation_context_id,
            is_last: false,
        }];
        let pdata_2 = vec![PDataValue {
            value_type: PDataValueType::Data,
            data: my_data[3000..6000].to_owned(),
            presentation_context_id,
            is_last: false,
        }];
        let pdata_3 = vec![PDataValue {
            value_type: PDataValueType::Data,
            data: my_data[6000..].to_owned(),
            presentation_context_id,
            is_last: true,
        }];

        let mut pdu_stream = VecDeque::new();

        // write some PDUs
        write_pdu(&mut pdu_stream, &Pdu::PData { data: pdata_1 }).unwrap();
        write_pdu(&mut pdu_stream, &Pdu::PData { data: pdata_2 }).unwrap();
        write_pdu(&mut pdu_stream, &Pdu::PData { data: pdata_3 }).unwrap();

        let mut buf = Vec::new();
        {
            let mut reader = PDataReader::new(&mut pdu_stream, MINIMUM_PDU_SIZE);
            reader.read_to_end(&mut buf).unwrap();
        }
        assert_eq!(buf, my_data);
    }
}
