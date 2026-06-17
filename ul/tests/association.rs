use dicom_dictionary_std::uids::{self, VERIFICATION};
use dicom_ul::{ClientAssociationOptions, Pdu, ServerAssociationOptions};
use rstest::rstest;
use std::time::Instant;

type Result<T, E = Box<dyn std::error::Error + Send + Sync + 'static>> = std::result::Result<T, E>;

const TIMEOUT_TOLERANCE: u64 = 25;

#[rstest]
#[case(100)]
#[case(500)]
#[case(1000)]
fn test_slow_association(#[case] timeout: u64) {
    let scu_init = ClientAssociationOptions::new()
        .with_abstract_syntax(VERIFICATION)
        .calling_ae_title("RANDOM")
        .read_timeout(std::time::Duration::from_secs(1))
        .connection_timeout(std::time::Duration::from_millis(timeout));

    let now = Instant::now();
    let _res = scu_init.establish_with("RANDOM@167.167.167.167:11111");
    let elapsed = now.elapsed();
    assert!(
        elapsed.as_millis() < (timeout + TIMEOUT_TOLERANCE).into(),
        "Elapsed time {}ms exceeded the timeout {}ms",
        elapsed.as_millis(),
        timeout
    );
}

#[cfg(feature = "async")]
#[rstest]
#[case(100)]
#[case(500)]
#[case(1000)]
#[tokio::test(flavor = "multi_thread")]
async fn test_slow_association_async(#[case] timeout: u64) {
    let scu_init = ClientAssociationOptions::new()
        .with_abstract_syntax(VERIFICATION)
        .calling_ae_title("RANDOM")
        .read_timeout(std::time::Duration::from_secs(1))
        .connection_timeout(std::time::Duration::from_millis(timeout));
    let now = Instant::now();
    let res = scu_init
        .establish_with_async("RANDOM@167.167.167.167:11111")
        .await;
    assert!(res.is_err());
    let elapsed = now.elapsed();
    println!("Elapsed time: {elapsed:?}");
    assert!(
        elapsed.as_millis() < (timeout + TIMEOUT_TOLERANCE).into(),
        "Elapsed time {}ms exceeded the timeout {}ms",
        elapsed.as_millis(),
        timeout
    );
}

/// Associations can be established
/// when identifying remote nodes by their application entity address.
#[test]
fn test_establish_via_ae_address() -> Result<()> {
    let listener = std::net::TcpListener::bind("localhost:0")?;
    let addr = listener.local_addr()?;
    let scp = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title("THIS-SCP")
        .with_abstract_syntax(VERIFICATION);

    // Spawn server thread
    let h = std::thread::spawn(move || -> Result<_> {
        let (stream, _addr) = listener.accept()?;
        let mut association = scp.establish(stream)?;

        // handle one release request
        let pdu = association.receive()?;
        assert_eq!(pdu, Pdu::ReleaseRQ);
        association.send(&Pdu::ReleaseRP)?;

        Ok(association)
    });

    // use bound socket address to create AE address
    let ae_address = format!("THIS-SCP@{addr}");

    // create SCU and establish association
    let association = ClientAssociationOptions::new()
        .calling_ae_title("THIS-SCU")
        .with_presentation_context(
            uids::VERIFICATION,
            vec![
                uids::IMPLICIT_VR_LITTLE_ENDIAN,
                uids::EXPLICIT_VR_LITTLE_ENDIAN,
            ],
        )
        .establish_with(&ae_address)?;

    // just release and finish
    association.release()?;

    let _ = h.join();

    Ok(())
}
