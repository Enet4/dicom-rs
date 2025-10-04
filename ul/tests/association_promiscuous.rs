use std::net::SocketAddr;

use dicom_ul::association::Error::NoAcceptedPresentationContexts;
use dicom_ul::pdu::PresentationContextResultReason::Acceptance;
use dicom_ul::pdu::{
    PresentationContextNegotiated, PresentationContextResultReason, UserVariableItem,
};
use dicom_ul::{
    ClientAssociationOptions, Pdu, ServerAssociationOptions, IMPLEMENTATION_CLASS_UID,
    IMPLEMENTATION_VERSION_NAME,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

const SCU_AE_TITLE: &str = "STORE-SCU";
const SCP_AE_TITLE: &str = "STORE-SCP";

const IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
const MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
const MR_IMAGE_STORAGE_RAW: &str = "1.2.840.10008.5.1.4.1.1.4\0";
const ULTRASOUND_IMAGE_STORAGE_RAW: &str = "1.2.840.10008.5.1.4.1.1.6.1\0";

fn spawn_scp(
    abstract_syntax_uids: &'static [&str],
    promiscuous: bool,
) -> Result<(std::thread::JoinHandle<Result<()>>, SocketAddr)> {
    let listener = std::net::TcpListener::bind("localhost:0")?;
    let addr = listener.local_addr()?;
    let mut options = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title(SCP_AE_TITLE)
        .promiscuous(promiscuous);

    for abstract_syntax_uid in abstract_syntax_uids {
        options = options.with_abstract_syntax(*abstract_syntax_uid);
    }

    let handle = std::thread::spawn(move || {
        let (stream, _addr) = listener.accept()?;
        let mut association = options.establish(stream)?;
        assert_eq!(
            association.presentation_contexts(),
            &[PresentationContextNegotiated {
                id: 1,
                reason: PresentationContextResultReason::Acceptance,
                transfer_syntax: IMPLICIT_VR_LE.to_string(),
                abstract_syntax: MR_IMAGE_STORAGE.to_string(),
            }]
        );

        let pdu = association.receive()?;
        assert_eq!(pdu, Pdu::ReleaseRQ);
        association.send(&Pdu::ReleaseRP)?;

        Ok(())
    });

    Ok((handle, addr))
}

#[cfg(feature = "async")]
async fn spawn_scp_async(
    abstract_syntax_uids: &'static [&str],
    promiscuous: bool,
) -> Result<(tokio::task::JoinHandle<Result<()>>, SocketAddr)> {
    let listener = tokio::net::TcpListener::bind("localhost:0").await?;
    let addr = listener.local_addr()?;
    let mut options = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title(SCP_AE_TITLE)
        .promiscuous(promiscuous);

    for abstract_syntax_uid in abstract_syntax_uids {
        options = options.with_abstract_syntax(*abstract_syntax_uid);
    }

    let handle = tokio::spawn(async move {
        let (stream, _addr) = listener.accept().await?;
        let mut association = options.establish_async(stream).await?;
        assert_eq!(
            association.presentation_contexts(),
            &[PresentationContextNegotiated {
                id: 1,
                reason: PresentationContextResultReason::Acceptance,
                transfer_syntax: IMPLICIT_VR_LE.to_string(),
                abstract_syntax: MR_IMAGE_STORAGE.to_string(),
            }]
        );

        let pdu = association.receive().await?;
        assert_eq!(pdu, Pdu::ReleaseRQ);
        association.send(&Pdu::ReleaseRP).await?;

        Ok(())
    });

    Ok((handle, addr))
}

#[test]
fn scu_scp_association_promiscuous_enabled() {
    // SCP is set to promiscuous mode - all abstract syntaxes are accepted
    let (scp_handle, scp_addr) = spawn_scp(&[], true).unwrap();

    let association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        .with_presentation_context(MR_IMAGE_STORAGE_RAW, vec![IMPLICIT_VR_LE])
        .establish(scp_addr)
        .unwrap();

    assert_eq!(
        association.user_variables(),
        &[
            UserVariableItem::MaxLength(16384),
            UserVariableItem::ImplementationClassUID(IMPLEMENTATION_CLASS_UID.to_string()),
            UserVariableItem::ImplementationVersionName(IMPLEMENTATION_VERSION_NAME.to_string())
        ]
    );
    assert_eq!(
        association.presentation_contexts(),
        &[PresentationContextNegotiated {
            id: 1,
            reason: Acceptance,
            transfer_syntax: IMPLICIT_VR_LE.to_string(),
            abstract_syntax: MR_IMAGE_STORAGE.to_string(),
        }]
    );
    assert_eq!(association.acceptor_max_pdu_length(), 16384);
    assert_eq!(association.requestor_max_pdu_length(), 16384);
    assert_eq!(association.read_timeout(), None);
    assert_eq!(association.write_timeout(), None);

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
async fn scu_scp_association_promiscuous_enabled_async() {
    // SCP is set to promiscuous mode - all abstract syntaxes are accepted
    let (scp_handle, scp_addr) = spawn_scp_async(&[], true).await.unwrap();

    let association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        .with_presentation_context(MR_IMAGE_STORAGE_RAW, vec![IMPLICIT_VR_LE])
        .establish_async(scp_addr)
        .await
        .unwrap();

    assert_eq!(
        association.user_variables(),
        &[
            UserVariableItem::MaxLength(16384),
            UserVariableItem::ImplementationClassUID(IMPLEMENTATION_CLASS_UID.to_string()),
            UserVariableItem::ImplementationVersionName(IMPLEMENTATION_VERSION_NAME.to_string())
        ]
    );
    assert_eq!(
        association.presentation_contexts(),
        &[PresentationContextNegotiated {
            id: 1,
            reason: Acceptance,
            transfer_syntax: IMPLICIT_VR_LE.to_string(),
            abstract_syntax: MR_IMAGE_STORAGE.to_string(),
        }]
    );
    assert_eq!(association.acceptor_max_pdu_length(), 16384);
    assert_eq!(association.requestor_max_pdu_length(), 16384);
    assert_eq!(association.read_timeout(), None);
    assert_eq!(association.write_timeout(), None);

    association
        .release()
        .await
        .expect("did not have a peaceful release");

    scp_handle
        .await
        .expect("SCP panicked")
        .expect("Error at the SCP");
}

#[test]
fn scu_scp_association_promiscuous_disabled() {
    // SCP only accepts Ultrasound Image Storage
    let (_scu_handle, scp_addr) = spawn_scp(&[ULTRASOUND_IMAGE_STORAGE_RAW], false).unwrap();

    let association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        .with_presentation_context(MR_IMAGE_STORAGE_RAW, vec![IMPLICIT_VR_LE])
        .establish(scp_addr);

    // Assert that no presentation context was accepted
    assert!(matches!(
        association,
        Err(NoAcceptedPresentationContexts { .. })
    ));
}

#[cfg(feature = "async")]
#[tokio::test(flavor = "multi_thread")]
async fn scu_scp_association_promiscuous_disabled_async() {
    // SCP only accepts Ultrasound Image Storage
    let (_scu_handle, scp_addr) = spawn_scp_async(&[ULTRASOUND_IMAGE_STORAGE_RAW], false)
        .await
        .unwrap();

    let association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        .with_presentation_context(MR_IMAGE_STORAGE_RAW, vec![IMPLICIT_VR_LE])
        .establish_async(scp_addr)
        .await;

    // Assert that no presentation context was accepted
    assert!(matches!(
        association,
        Err(NoAcceptedPresentationContexts { .. })
    ));
}
