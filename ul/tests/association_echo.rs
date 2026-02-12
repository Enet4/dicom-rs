use dicom_ul::{
    ServerAssociation, association::{Association, Error, SyncAssociation, client::ClientAssociationOptions, server::ServerAssociationOptions}, pdu::{
        PDataValue, PDataValueType, Pdu, PresentationContextNegotiated,
        PresentationContextResultReason,
    }
};
#[cfg(feature = "async")]
use dicom_ul::association::AsyncServerAssociation;
use std::io::Write;

#[cfg(feature = "async")]
use tokio::io::AsyncWriteExt;

use std::net::SocketAddr;

// Check rather arbitrary maximum PDU lengths, also different for server and client
const HI_PDU_LEN: usize = 7890;
const LO_PDU_LEN: usize = 5678;
const PDV_HDR_LEN: usize = 6;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

static SCU_AE_TITLE: &str = "ECHO-SCU";
static SCP_AE_TITLE: &str = "ECHO-SCP";

static IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
static EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
static JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
static VERIFICATION_SOP_CLASS: &str = "1.2.840.10008.1.1";
static DIGITAL_MG_STORAGE_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.1.2";

// Return a PData PDU with one PDV which has a payload of the given length.
// It's a "bogus packet" because the payload is filled with zeros instead of
// being a valid DICOM object.
fn bogus_packet(len: usize) -> Pdu {
    Pdu::PData {
        data: vec![PDataValue {
            presentation_context_id: 1,
            value_type: PDataValueType::Command,
            is_last: true,
            data: vec![0_u8; len],
        }],
    }
}

fn spawn_scp(
    max_server_pdu_len: usize,
    max_client_pdu_len: usize,
) -> Result<(std::thread::JoinHandle<Result<ServerAssociation<std::net::TcpStream>>>, SocketAddr)> {
    let listener = std::net::TcpListener::bind("localhost:0")?;
    let addr = listener.local_addr()?;
    let scp = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title(SCP_AE_TITLE)
        .max_pdu_length(max_server_pdu_len as u32)
        .with_abstract_syntax(VERIFICATION_SOP_CLASS);

    let h = std::thread::spawn(move || -> Result<_> {
        let (stream, _addr) = listener.accept()?;
        let mut association = scp.establish(stream)?;

        assert_eq!(
            association.presentation_contexts(),
            &[
                PresentationContextNegotiated {
                    id: 1,
                    reason: PresentationContextResultReason::Acceptance,
                    transfer_syntax: IMPLICIT_VR_LE.to_string(),
                    abstract_syntax: VERIFICATION_SOP_CLASS.to_string(),
                },
                PresentationContextNegotiated {
                    id: 3,
                    reason: PresentationContextResultReason::AbstractSyntaxNotSupported,
                    transfer_syntax: IMPLICIT_VR_LE.to_string(),
                    abstract_syntax: DIGITAL_MG_STORAGE_SOP_CLASS.to_string(),
                }
            ],
        );

        assert_eq!(
            association.requestor_max_pdu_length(),
            max_client_pdu_len as u32
        );
        assert_eq!(
            association.acceptor_max_pdu_length(),
            max_server_pdu_len as u32
        );

        // handle one bogus payload
        let pdu = association.receive()?;
        let data = match pdu {
            Pdu::PData { ref data } => data,
            other => panic!("Unexpected packet type: {:?}", other),
        };
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].data.len(), max_server_pdu_len - PDV_HDR_LEN);

        // Create a bogus payload which fills the PDU to the max.
        // Take into account the PDU and PDV header lengths for that purpose.
        let filler_len = max_client_pdu_len - PDV_HDR_LEN;
        let mut packet = bogus_packet(filler_len);

        // send one bogus response
        association.send(&packet).expect("failed sending packet");

        // Add 1 byte to the payload to exceed maximum length
        if let Pdu::PData { ref mut data } = packet {
            data[0].data.push(0);
        }

        match association.send(&packet) {
            Err(Error::SendTooLongPdu { .. }) => (),
            e => panic!("Expected SendTooLongPdu but didn't happen: {:?}", e),
        }

        // Test send_pdata() fragmentation of the client; we should receive two packets
        // First packet
        match association.receive() {
            Ok(Pdu::PData { data }) => {
                assert_eq!(data.len(), 1);
                assert_eq!(data[0].data.len(), max_server_pdu_len - PDV_HDR_LEN);
            }
            Ok(other_pdus) => { panic!("Unknown PDU: {:?}", other_pdus); }
            Err(err) => { panic!("Receive returned error {:?}", err); }
        }
        // Second packet
        match association.receive() {
            Ok(Pdu::PData { data }) => {
                assert_eq!(data.len(), 1);
                assert_eq!(data[0].data.len(), 2);
            }
            Ok(other_pdus) => { panic!("Unknown PDU: {:?}", other_pdus); }
            Err(err) => { panic!("Receive returned error {:?}", err); }
        }
        // Let the client test our send_pdata() fragmentation for us
        {
            // Send two more bytes than fit in a PDU
            let filler_len = max_client_pdu_len - PDV_HDR_LEN + 2;
            let buf = vec![0_u8; filler_len];
            let mut sender = association.send_pdata(1);
            // This should split the data in two packets
            sender.write_all(&buf).expect("Error sending fragmented data");
        }

        // handle one release request
        let pdu = association.receive()?;
        assert_eq!(pdu, Pdu::ReleaseRQ);
        association.send(&Pdu::ReleaseRP)?;

        Ok(association)
    });
    Ok((h, addr))
}

#[cfg(feature = "async")]
async fn spawn_scp_async(
    max_server_pdu_len: usize,
    max_client_pdu_len: usize,
) -> Result<(tokio::task::JoinHandle<Result<AsyncServerAssociation<tokio::net::TcpStream>>>, SocketAddr)> {
    let listener = tokio::net::TcpListener::bind("localhost:0").await?;
    let addr = listener.local_addr()?;
    let scp = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title(SCP_AE_TITLE)
        .max_pdu_length(max_server_pdu_len as u32)
        .with_abstract_syntax(VERIFICATION_SOP_CLASS);

    let h = tokio::spawn(async move {
        use dicom_ul::association::AsyncAssociation;

        let (stream, _addr) = listener.accept().await?;
        let mut association = scp.establish_async(stream).await?;

        assert_eq!(
            association.presentation_contexts(),
            &[
                PresentationContextNegotiated {
                    id: 1,
                    reason: PresentationContextResultReason::Acceptance,
                    transfer_syntax: IMPLICIT_VR_LE.to_string(),
                    abstract_syntax: VERIFICATION_SOP_CLASS.to_string(),
                },
                PresentationContextNegotiated {
                    id: 3,
                    reason: PresentationContextResultReason::AbstractSyntaxNotSupported,
                    transfer_syntax: IMPLICIT_VR_LE.to_string(),
                    abstract_syntax: DIGITAL_MG_STORAGE_SOP_CLASS.to_string(),
                }
            ],
        );

        assert_eq!(
            association.requestor_max_pdu_length(),
            max_client_pdu_len as u32
        );
        assert_eq!(
            association.acceptor_max_pdu_length(),
            max_server_pdu_len as u32
        );

        // handle one bogus payload
        let pdu = association.receive().await?;
        let data = match pdu {
            Pdu::PData { ref data } => data,
            other => panic!("Unexpected packet type: {:?}", other),
        };
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].data.len(), max_server_pdu_len - PDV_HDR_LEN);

        // Create a bogus payload which fills the PDU to the max.
        // Take into account the PDU and PDV header lengths for that purpose.
        let filler_len = max_client_pdu_len - PDV_HDR_LEN;
        let mut packet = bogus_packet(filler_len);

        // send one bogus response
        association
            .send(&packet)
            .await
            .expect("failed sending packet");

        if let Pdu::PData { ref mut data } = packet {
            // Add 1 byte to the payload to exceed maximum length
            data[0].data.push(0);
        }

        match association.send(&packet).await {
            Err(Error::SendTooLongPdu { .. }) => (),
            e => panic!("Expected SendTooLongPdu but didn't happen: {:?}", e),
        }

        // Test send_pdata() fragmentation of the client; we should receive two packets
        // First packet
        match association.receive().await {
            Ok(Pdu::PData { data }) => {
                assert_eq!(data.len(), 1);
                assert_eq!(data[0].data.len(), max_server_pdu_len - PDV_HDR_LEN);
            }
            Ok(other_pdus) => { panic!("Unknown PDU: {:?}", other_pdus); }
            Err(err) => { panic!("Receive returned error {:?}", err); }
        }
        // Second packet
        match association.receive().await {
            Ok(Pdu::PData { data }) => {
                assert_eq!(data.len(), 1);
                assert_eq!(data[0].data.len(), 2);
            }
            Ok(other_pdus) => { panic!("Unknown PDU: {:?}", other_pdus); }
            Err(err) => { panic!("Receive returned error {:?}", err); }
        }
        // Let the client test our send_pdata() fragmentation
        {
            // Send two more bytes than fit in a PDU
            let filler_len = max_client_pdu_len - PDV_HDR_LEN + 2;
            let buf = vec![0_u8; filler_len];
            let mut sender = association.send_pdata(1);
            // This should split the data in two packets
            sender.write_all(&buf).await.expect("Error sending fragmented data");
        }

        // handle one release request
        let pdu = association.receive().await?;
        assert_eq!(pdu, Pdu::ReleaseRQ);
        association.send(&Pdu::ReleaseRP).await?;

        Ok(association)
    });
    Ok((h, addr))
}

/// Run an SCP and an SCU concurrently, negotiate an association and release it.
#[test]
fn scu_scp_association_test() {
    for max_is_client in [false, true] {
        run_scu_scp_association_test(max_is_client);
    }
}

fn run_scu_scp_association_test(max_is_client: bool) {
    let (max_client_pdu_len, max_server_pdu_len) = if max_is_client {
        (HI_PDU_LEN, LO_PDU_LEN)
    } else {
        (LO_PDU_LEN, HI_PDU_LEN)
    };
    let (scp_handle, scp_addr) = spawn_scp(max_server_pdu_len, max_client_pdu_len).unwrap();

    let mut association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        .with_presentation_context(VERIFICATION_SOP_CLASS, vec![IMPLICIT_VR_LE, EXPLICIT_VR_LE])
        .with_presentation_context(
            DIGITAL_MG_STORAGE_SOP_CLASS,
            vec![IMPLICIT_VR_LE, EXPLICIT_VR_LE, JPEG_BASELINE],
        )
        .max_pdu_length(max_client_pdu_len as u32)
        .establish(scp_addr)
        .unwrap();

    assert_eq!(
        association.requestor_max_pdu_length(),
        max_client_pdu_len as u32
    );
    assert_eq!(
        association.acceptor_max_pdu_length(),
        max_server_pdu_len as u32
    );

    // Create a bogus payload which fills the PDU to the max.
    // Take into account the PDU and PDV header lengths for that purpose.
    let filler_len = max_server_pdu_len - PDV_HDR_LEN;
    let mut packet = bogus_packet(filler_len);

    association.send(&packet).expect("failed sending packet");

    let pdu = association.receive().expect("can't receive response");
    match pdu {
        Pdu::PData { .. } => (),
        _ => panic!("unexpected response packet type"),
    }

    // Add 1 byte to the payload to exceed maximum length
    if let Pdu::PData { ref mut data } = packet {
        data[0].data.push(0);
    }
    match association.send(&packet) {
        Err(Error::SendTooLongPdu { .. }) => (),
        e => panic!("Expected SendTooLongPdu but didn't happen: {:?}", e),
    }

    // Let the server test our send_pdata() fragmentation for us
    {
        // Send two more bytes than fit in a PDU
        let filler_len = max_server_pdu_len - PDV_HDR_LEN + 2;
        let buf = vec![0_u8; filler_len];
        let mut sender = association.send_pdata(1);
        // This should split the data in two packets
        sender.write_all(&buf).expect("Error sending fragmented data");
    }
    // Test send_pdata() fragmentation of the server; we should receive two packets
    // First packet
    match association.receive() {
        Ok(Pdu::PData { data }) => {
            assert_eq!(data.len(), 1);
            assert_eq!(data[0].data.len(), max_client_pdu_len - PDV_HDR_LEN);
        }
        Ok(other_pdus) => { panic!("Unknown PDU: {:?}", other_pdus); }
        Err(err) => { panic!("Receive returned error {:?}", err); }
    }
    // Second packet
    match association.receive() {
        Ok(Pdu::PData { data }) => {
            assert_eq!(data.len(), 1);
            assert_eq!(data[0].data.len(), 2);
        }
        Ok(other_pdus) => { panic!("Unknown PDU: {:?}", other_pdus); }
        Err(err) => { panic!("Receive returned error {:?}", err); }
    }

    association
        .release()
        .expect("did not have a peaceful release");

    scp_handle
        .join()
        .expect("SCP panicked")
        .expect("Error at the SCP");
}

#[cfg(feature = "async")]
#[tokio::test(flavor = "multi_thread")]
async fn scu_scp_association_test_async() {
    for max_is_client in [false, true] {
        run_scu_scp_association_test_async(max_is_client).await;
    }
}

#[cfg(feature = "async")]
async fn run_scu_scp_association_test_async(max_is_client: bool) {
    use dicom_ul::association::AsyncAssociation;

    let (max_client_pdu_len, max_server_pdu_len) = if max_is_client {
        (HI_PDU_LEN, LO_PDU_LEN)
    } else {
        (LO_PDU_LEN, HI_PDU_LEN)
    };
    let (scp_handle, scp_addr) = spawn_scp_async(max_server_pdu_len, max_client_pdu_len)
        .await
        .unwrap();

    let mut association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        .with_presentation_context(VERIFICATION_SOP_CLASS, vec![IMPLICIT_VR_LE, EXPLICIT_VR_LE])
        .with_presentation_context(
            DIGITAL_MG_STORAGE_SOP_CLASS,
            vec![IMPLICIT_VR_LE, EXPLICIT_VR_LE, JPEG_BASELINE],
        )
        .max_pdu_length(max_client_pdu_len as u32)
        .establish_async(scp_addr)
        .await
        .unwrap();

    assert_eq!(
        association.requestor_max_pdu_length(),
        max_client_pdu_len as u32
    );
    assert_eq!(
        association.acceptor_max_pdu_length(),
        max_server_pdu_len as u32
    );

    // Create a bogus payload which fills the PDU to the max.
    // Take into account the PDU and PDV header lengths for that purpose.
    let filler_len = max_server_pdu_len - PDV_HDR_LEN;
    let mut packet = bogus_packet(filler_len);

    association
        .send(&packet)
        .await
        .expect("failed sending packet (async)");

    let pdu = association
        .receive()
        .await
        .expect("can't receive response (async)");
    match pdu {
        Pdu::PData { .. } => (),
        _ => panic!("unexpected response packet type (async)"),
    }

    // Add 1 byte to the payload to exceed maximum length
    if let Pdu::PData { ref mut data } = packet {
        data[0].data.push(0);
    }
    match association.send(&packet).await {
        Err(Error::SendTooLongPdu { .. }) => (),
        e => panic!("Expected SendTooLongPdu but didn't happen (async): {:?}", e),
    }

    // Let the server test our send_pdata() fragmentation for us
    {
        // Send two more bytes than fit in a PDU
        let filler_len = max_server_pdu_len - PDV_HDR_LEN + 2;
        let buf = vec![0_u8; filler_len];
        let mut sender = association.send_pdata(1);
        // This should split the data in two packets
        sender.write_all(&buf).await.expect("Error sending fragmented data");
    }
    // Test send_pdata() fragmentation of the server; we should receive two packets
    // First packet
    match association.receive().await {
        Ok(Pdu::PData { data }) => {
            assert_eq!(data.len(), 1);
            assert_eq!(data[0].data.len(), max_client_pdu_len - PDV_HDR_LEN);
        }
        Ok(other_pdus) => { panic!("Unknown PDU: {:?}", other_pdus); }
        Err(err) => { panic!("Receive returned error {:?}", err); }
    }
    // Second packet
    match association.receive().await {
        Ok(Pdu::PData { data }) => {
            assert_eq!(data.len(), 1);
            assert_eq!(data[0].data.len(), 2);
        }
        Ok(other_pdus) => { panic!("Unknown PDU: {:?}", other_pdus); }
        Err(err) => { panic!("Receive returned error {:?}", err); }
    }

    association
        .release()
        .await
        .expect("did not have a peaceful release (async)");

    scp_handle
        .await
        .expect("SCP panicked (async)")
        .expect("Error at the SCP (async)");
}
