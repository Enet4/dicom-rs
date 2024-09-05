use dicom_ul::{
    association::client::ClientAssociationOptions,
    pdu::{Pdu, PresentationContextResult, PresentationContextResultReason},
};

use std::net::SocketAddr;

use dicom_ul::association::server::ServerAssociationOptions;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

static SCU_AE_TITLE: &str = "ECHO-SCU";
static SCP_AE_TITLE: &str = "ECHO-SCP";

static IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
static EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
static JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
static VERIFICATION_SOP_CLASS: &str = "1.2.840.10008.1.1";
static DIGITAL_MG_STORAGE_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.1.2";

fn spawn_scp() -> Result<(std::thread::JoinHandle<Result<()>>, SocketAddr)> {
    let listener = std::net::TcpListener::bind("localhost:0")?;
    let addr = listener.local_addr()?;
    let scp = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title(SCP_AE_TITLE)
        .with_abstract_syntax(VERIFICATION_SOP_CLASS);

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
                PresentationContextResult {
                    id: 3,
                    reason: PresentationContextResultReason::AbstractSyntaxNotSupported,
                    transfer_syntax: IMPLICIT_VR_LE.to_string(),
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
        .with_abstract_syntax(VERIFICATION_SOP_CLASS);

    let h = tokio::spawn(async move {
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
                PresentationContextResult {
                    id: 2,
                    reason: PresentationContextResultReason::AbstractSyntaxNotSupported,
                    transfer_syntax: IMPLICIT_VR_LE.to_string(),
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

/// Run an SCP and an SCU concurrently, negotiate an association and release it.
#[test]
fn scu_scp_association_test() {
    let (scp_handle, scp_addr) = spawn_scp().unwrap();

    let association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        .with_presentation_context(VERIFICATION_SOP_CLASS, vec![IMPLICIT_VR_LE, EXPLICIT_VR_LE])
        .with_presentation_context(
            DIGITAL_MG_STORAGE_SOP_CLASS,
            vec![IMPLICIT_VR_LE, EXPLICIT_VR_LE, JPEG_BASELINE],
        )
        .establish(scp_addr)
        .unwrap();

    association
        .release()
        .expect("did not have a peaceful release");

    scp_handle
        .join()
        .expect("SCP panicked")
        .expect("Error at the SCP");
}

#[tokio::test(flavor = "multi_thread")]
async fn scu_scp_asociation_test() {
    let (scp_handle, scp_addr) = spawn_scp_async().await.unwrap();

    let association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        .with_presentation_context(VERIFICATION_SOP_CLASS, vec![IMPLICIT_VR_LE, EXPLICIT_VR_LE])
        .with_presentation_context(
            DIGITAL_MG_STORAGE_SOP_CLASS,
            vec![IMPLICIT_VR_LE, EXPLICIT_VR_LE, JPEG_BASELINE],
        )
        .establish_async(scp_addr)
        .await
        .unwrap();

    association
        .release()
        .await
        .expect("did not have a peaceful release");

    scp_handle
        .await
        .expect("SCP panicked")
        .expect("Error at the SCP");
}
