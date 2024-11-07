use dicom_dictionary_std::uids::VERIFICATION;
use dicom_ul::ClientAssociationOptions;
use rstest::rstest;
use std::time::Instant;

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
    assert!(elapsed.as_millis() < (timeout + 10).into());
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
    println!("Elapsed time: {:?}", elapsed);
    assert!(elapsed.as_millis() < (timeout + 10).into());
}
