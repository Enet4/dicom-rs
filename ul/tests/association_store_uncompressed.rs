//! A test suite involving a store SCP
//! which only accepts uncompressed transfer syntaxes

use dicom_ul::{
    association::client::ClientAssociationOptions,
    pdu::{Pdu, PresentationContextResult, PresentationContextResultReason},
};
use std::net::SocketAddr;

use dicom_ul::association::server::ServerAssociationOptions;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

static SCU_AE_TITLE: &str = "STORE-SCU";
static SCP_AE_TITLE: &str = "STORE-SCP";

static EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
static IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
static JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
// raw UID with even padding
// as potentially provided by DICOM objects
static MR_IMAGE_STORAGE_RAW: &str = "1.2.840.10008.5.1.4.1.1.4\0";
static MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
static DIGITAL_MG_STORAGE_SOP_CLASS_RAW: &str = "1.2.840.10008.5.1.4.1.1.1.2\0";
static DIGITAL_MG_STORAGE_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.1.2";

fn spawn_scp() -> Result<(std::thread::JoinHandle<Result<()>>, SocketAddr)> {
    let listener = std::net::TcpListener::bind("localhost:0")?;
    let addr = listener.local_addr()?;
    let scp = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title(SCP_AE_TITLE)
        .with_abstract_syntax(MR_IMAGE_STORAGE)
        .with_abstract_syntax(DIGITAL_MG_STORAGE_SOP_CLASS)
        .with_transfer_syntax(EXPLICIT_VR_LE)
        .with_transfer_syntax(IMPLICIT_VR_LE);

    let h = std::thread::spawn(move || -> Result<()> {
        let (stream, _addr) = listener.accept()?;
        let mut association = scp.establish(stream)?;

        assert_eq!(
            association.presentation_contexts(),
            &[
                PresentationContextResult {
                    id: 1,
                    reason: PresentationContextResultReason::Acceptance,
                    transfer_syntax: IMPLICIT_VR_LE.to_string(),
                },
                // should always pick Explicit VR LE
                // because JPEG baseline was not explicitly enabled in SCP
                PresentationContextResult {
                    id: 3,
                    reason: PresentationContextResultReason::Acceptance,
                    transfer_syntax: EXPLICIT_VR_LE.to_string(),
                }
            ],
        );

        // handle one release request
        let pdu = association.receive()?;
        assert_eq!(pdu, Pdu::ReleaseRQ);
        association.send(&Pdu::ReleaseRP)?;

        Ok(())
    });
    Ok((h, addr))
}

async fn spawn_scp_async() -> Result<(tokio::task::JoinHandle<Result<()>>, SocketAddr)> {
    let listener = tokio::net::TcpListener::bind("localhost:0").await?;
    let addr = listener.local_addr()?;
    let scp = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title(SCP_AE_TITLE)
        .with_abstract_syntax(MR_IMAGE_STORAGE)
        .with_abstract_syntax(DIGITAL_MG_STORAGE_SOP_CLASS)
        .with_transfer_syntax(EXPLICIT_VR_LE)
        .with_transfer_syntax(IMPLICIT_VR_LE);

    let h = tokio::task::spawn(async move {
        let (stream, _addr) = listener.accept().await?;
        let mut association = scp.establish_async(stream).await?;

        assert_eq!(
            association.presentation_contexts(),
            &[
                PresentationContextResult {
                    id: 1,
                    reason: PresentationContextResultReason::Acceptance,
                    transfer_syntax: IMPLICIT_VR_LE.to_string(),
                },
                // should always pick Explicit VR LE
                // because JPEG baseline was not explicitly enabled in SCP
                PresentationContextResult {
                    id: 2,
                    reason: PresentationContextResultReason::Acceptance,
                    transfer_syntax: EXPLICIT_VR_LE.to_string(),
                }
            ],
        );

        // handle one release request
        let pdu = association.receive().await?;
        assert_eq!(pdu, Pdu::ReleaseRQ);
        association.send(&Pdu::ReleaseRP).await?;

        Ok(())
    });
    Ok((h, addr))
}

/// Run an SCP and an SCU concurrently,
/// negotiate an association with distinct transfer syntaxes
/// and release it.
#[test]
fn scu_scp_association_uncompressed() {
    let (scp_handle, scp_addr) = spawn_scp().unwrap();

    let association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        .with_presentation_context(MR_IMAGE_STORAGE_RAW, vec![IMPLICIT_VR_LE])
        // MG storage, JPEG baseline
        .with_presentation_context(
            DIGITAL_MG_STORAGE_SOP_CLASS_RAW,
            vec![JPEG_BASELINE, EXPLICIT_VR_LE, IMPLICIT_VR_LE],
        )
        .establish(scp_addr)
        .unwrap();

    for pc in association.presentation_contexts() {
        match pc.id {
            // guaranteed to be MR image storage
            1 => {
                // only one option provided
                assert_eq!(pc.transfer_syntax, IMPLICIT_VR_LE);
            }
            // guaranteed to be MG image storage
            3 => {
                // server picked this one because it did not accept JPEG baseline
                assert_eq!(pc.transfer_syntax, EXPLICIT_VR_LE);
            }
            id => panic!("unexpected presentation context ID {}", id),
        }
    }

    association
        .release()
        .expect("did not have a peaceful release");

    scp_handle
        .join()
        .expect("SCP panicked")
        .expect("Error at the SCP");
}

#[tokio::test(flavor = "multi_thread")]
async fn scu_scp_association_uncompressed_async() {
    let (scp_handle, scp_addr) = spawn_scp_async().await.unwrap();

    let association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        .with_presentation_context(MR_IMAGE_STORAGE_RAW, vec![IMPLICIT_VR_LE])
        // MG storage, JPEG baseline
        .with_presentation_context(
            DIGITAL_MG_STORAGE_SOP_CLASS_RAW,
            vec![JPEG_BASELINE, EXPLICIT_VR_LE, IMPLICIT_VR_LE],
        )
        .establish_async(scp_addr)
        .await
        .unwrap();

    for pc in association.presentation_contexts() {
        match pc.id {
            // guaranteed to be MR image storage
            1 => {
                // only one option provided
                assert_eq!(pc.transfer_syntax, IMPLICIT_VR_LE);
            }
            // guaranteed to be MG image storage
            2 => {
                // server picked this one because it did not accept JPEG baseline
                assert_eq!(pc.transfer_syntax, EXPLICIT_VR_LE);
            }
            id => panic!("unexpected presentation context ID {}", id),
        }
    }

    association
        .release()
        .await
        .expect("did not have a peaceful release");

    scp_handle
        .await
        .expect("SCP panicked")
        .expect("Error at the SCP");
}
