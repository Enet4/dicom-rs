#[cfg(feature = "async")]
mod async_pdata_tests {
    use std::{collections::VecDeque, pin::Pin, task::{Poll, Context}};

    use dicom_ul::{Pdu, ServerAssociationOptions, association::{AsyncPDataWriter, ClientAssociationOptions}, pdu::{DEFAULT_MAX_PDU}, read_pdu};
    use tokio::io::{AsyncWriteExt, AsyncReadExt, AsyncWrite};
    use rstest::rstest;

    static IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
    static MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
    type Result<T, E = Box<dyn std::error::Error + Send + Sync + 'static>> = std::result::Result<T, E>;


    struct ControlledStream {
        pub inner: Vec<u8>,
        control: VecDeque<Option<usize>>
    }

    impl AsyncWrite for ControlledStream {
        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>>
        {
            match self.control.pop_front(){
                Some(Some(n)) => {
                    let taken = n.min(buf.len());
                    self.inner.extend_from_slice(&buf[..taken]);
                    Poll::Ready(Ok(taken))
                },
                Some(None) => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending
                },
                None => {
                    self.inner.extend_from_slice(buf);
                    return Poll::Ready(Ok(buf.len()))
                },
            }
        }
    }

    fn collect_pdata_bytes(bytes: &[u8]) -> Vec<u8> {
        let mut cursor = bytes;
        let mut out = Vec::new();
        let mut i = 0;
        loop {
            match read_pdu(&mut cursor, DEFAULT_MAX_PDU, true).unwrap() {
                Some(Pdu::PData { data }) => {
                    let outlen = out.len();
                    for v in data { out.extend(v.data); }
                    let added = out.len() - outlen;
                    println!("Received PDU {:?}, len: {:?}",i, added);
                    i += 1;
                }
                Some(other) => panic!("unexpected non-PData PDU: {other:?}"),
                None => break,
            }
        }
        out
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_async_pdata_writer() -> Result<(), Box<dyn std::error::Error>> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();
        let server_options = ServerAssociationOptions::new()
            .accept_called_ae_title()
            .ae_title("TEST_SCP")
            .with_abstract_syntax(MR_IMAGE_STORAGE);
        let server_handle: tokio::task::JoinHandle<()> = tokio::spawn(async move {
            let (stream, _) = listener
                .accept()
                .await
                .unwrap();
            let mut association = server_options
                .establish_async(stream)
                .await
                .unwrap();
            let mut buf = Vec::new();
            let mut reader = association.receive_pdata();
            reader.read_to_end(&mut buf).await.unwrap();
            assert_eq!(buf.len(), 10 * 1024 * 1024);
            println!("Server received {} bytes", buf.len());
        });
        let mut scu = ClientAssociationOptions::new()
            .calling_ae_title("TEST_SCU")
            .called_ae_title("TEST_SCP")
            .with_presentation_context(MR_IMAGE_STORAGE, vec![IMPLICIT_VR_LE])
            .establish_async(server_addr)
            .await?;

        let pc_id = scu.presentation_contexts()[0].id;
        let mut pdata = scu.send_pdata(pc_id);

        // Any data larger than negotiated Max PDU Length triggers the hang
        let large_object = vec![0u8; 10 * 1024 * 1024]; // 3 MB

        // This line never returns
        pdata.write_all(&large_object).await?;
        pdata.finish().await?;

        server_handle.await.unwrap();
        Ok(())
    }

    #[rstest]
    #[case(vec![Some(200), Some(100), None, Some(100)])]
    #[case(vec![Some(2000), Some(1000), None, Some(1000)])]
    #[case(vec![Some(2000), Some(1000), None, None, None, Some(1000)])]
    #[case(vec![Some(2000), Some(1000), None, Some(1000), None, None, Some(1000)])]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_partial_write(#[case] control: Vec<Option<usize>>){
        let mut writer = ControlledStream {
            inner: vec![],
            control: VecDeque::from(control),
        };
        let test_buffer: Vec<u8> = vec![0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7].into_iter().cycle().take(1048576).collect();
        {
            let mut pdata_writer = AsyncPDataWriter::new(&mut writer, 1, DEFAULT_MAX_PDU);
            pdata_writer.write_all(&test_buffer).await.expect("Error raised in write_all")
        }
        let res = collect_pdata_bytes(&writer.inner);
        assert_eq!(res, test_buffer);
    }
}