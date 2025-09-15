use dicom_dictionary_std::uids::VERIFICATION;
use dicom_ul::ClientAssociationOptions;
use rstest::rstest;
use std::time::Instant;

#[cfg(feature = "tls")]
fn ensure_test_certs() -> Result<(), Box<dyn std::error::Error>> {
    use rustls_cert_gen::CertificateBuilder;
    use rcgen::SanType;
    use std::{convert::TryInto, net::IpAddr, str::FromStr, path::PathBuf};

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("certs");
    let cert_names = vec!["ca.pem", "ca.key.pem", "client.pem", "client.key.pem", "server.pem", "server.key.pem"];
    if cert_names.iter().all(|path| out_dir.join(path).exists()){
        println!("All certs exist, exiting");
        return Ok(());
    }

    // Create output directory
    std::fs::create_dir_all(&out_dir)?;

    // Generate Certificate Authority (CA)
    let ca = CertificateBuilder::new()
		.certificate_authority()
		.country_name(&"US")?
		.organization_name(&"DICOM-RS-CA")
		.build()?;

    // Write CA certificate and private key to `../certs/ca.pem` and `../certs/ca.key.pem`
    ca.serialize_pem().write(&out_dir, "ca")?;

    // Generate Client keypair
    let mut client = CertificateBuilder::new()
		.end_entity()
		.common_name(&"DICOM-RS-CLIENT")
		.subject_alternative_names(vec![SanType::IpAddress(IpAddr::from_str("127.0.0.1")?), SanType::DnsName("localhost".try_into()?)]);
    client.client_auth();

    client
        .build(&ca)?
        .serialize_pem().write(&out_dir, "client")?;

    // Generate Server keypair
    let mut server = CertificateBuilder::new()
		.end_entity()
		.common_name(&"DICOM-RS-SERVER")
		.subject_alternative_names(vec![SanType::IpAddress(IpAddr::from_str("127.0.0.1")?), SanType::DnsName("localhost".try_into()?)]);
    server.server_auth();

    server
        .build(&ca)?
        .serialize_pem().write(&out_dir, "server")?;

    Ok(())
}

#[cfg(feature = "tls")]
use std::sync::Arc;
#[cfg(feature = "tls")]
use rustls::{
    ServerConfig, ClientConfig, 
    pki_types::{CertificateDer, PrivateKeyDer},
    RootCertStore,
    server::WebPkiClientVerifier
};
#[cfg(feature = "tls")]
use dicom_ul::association::{server::ServerAssociationOptions, Association, SyncAssociation};

#[cfg(feature = "async-tls")]
use dicom_ul::association::AsyncAssociation;

const TIMEOUT_TOLERANCE: u64 = 25;

#[cfg(feature = "tls")]
/// Create a test TLS server configuration
fn create_test_config() -> Result<(Arc<ServerConfig>, Arc<ClientConfig>), Box<dyn std::error::Error>> {
    use rustls::pki_types::pem::PemObject;
    use std::path::PathBuf;
    ensure_test_certs()?;
    

    let ca_cert_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("certs/ca.pem");
    let ca_cert = CertificateDer::from_pem_slice(&std::fs::read(ca_cert_path)?)
        .expect("Failed to load CA cert");

    let client_cert_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("certs/client.pem");
    let client_cert = CertificateDer::from_pem_slice(&std::fs::read(client_cert_path)?)
        .expect("Failed to load client cert");

    let client_key_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("certs/client.key.pem");
    let client_private_key = PrivateKeyDer::from_pem_slice(&std::fs::read(client_key_path)?)
        .expect("Failed to load client private key");

    let server_cert_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("certs/server.pem");
    let server_cert = CertificateDer::from_pem_slice(&std::fs::read(server_cert_path)?)
        .expect("Failed to load server cert");

    let server_key_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("certs/server.key.pem");
    let server_private_key = PrivateKeyDer::from_pem_slice(&std::fs::read(server_key_path)?)
        .expect("Failed to load server private key");

    // Create a root cert store for the client which includes the server certificate
    let mut certs = RootCertStore::empty();
    certs.add_parsable_certificates(vec![ca_cert.clone()]);
    let certs = Arc::new(certs);
    
    // Server configuration.
    // Creates a server config that requires client authentication (mutual TLS) using 
    // webpki for certificate verification.
    let server_config = ServerConfig::builder()
        .with_client_cert_verifier(
            WebPkiClientVerifier::builder(certs.clone())
                .build()
                .expect("Failed to create client cert verifier")
        )
        .with_single_cert(vec![server_cert.clone(), ca_cert.clone()], server_private_key)
        .expect("Failed to create server TLS config");

    let config = ClientConfig::builder()
        .with_root_certificates(certs)
        .with_client_auth_cert(vec![client_cert, ca_cert], client_private_key)
        .expect("Failed to create client TLS config");

    Ok((Arc::new(server_config), Arc::new(config)))
}

#[cfg(feature = "tls")]
#[test]
fn test_tls_connection_sync() {

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind listener");
    let server_addr = listener.local_addr().expect("Failed to get local address");
    
    // Server configuration
    let (server_tls_config, client_tls_config) = create_test_config().expect("Failed to create test config");
    let server_options = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title("TLS-SCP")
        .with_abstract_syntax(VERIFICATION)
        .tls_config((*server_tls_config).clone());
    
    // Spawn server thread
    let server_handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("Failed to accept connection");
        let mut association = server_options.establish_tls(stream)
            .expect("Failed to establish TLS association");
        
        // Verify we can access association properties
        assert_eq!(association.peer_ae_title(), "TLS-SCU");
        assert!(!association.presentation_contexts().is_empty());
        
        // Wait for a release request
        let pdu = association.receive().expect("Failed to receive PDU");
        if let dicom_ul::Pdu::ReleaseRQ = pdu {
            association.send(&dicom_ul::Pdu::ReleaseRP).expect("Failed to send ReleaseRP");
        }
        association
    });
    
    // Give server time to start
    std::thread::sleep(std::time::Duration::from_millis(50));
    
    // Client configuration
    let client_options = ClientAssociationOptions::new()
        .calling_ae_title("TLS-SCU")
        .called_ae_title("TLS-SCP")
        .with_abstract_syntax(VERIFICATION)
        .server_name("localhost")
        .tls_config((*client_tls_config).clone());
    
    // Establish TLS connection
    let association = client_options.establish_tls(server_addr)
        .expect("Failed to establish TLS association");
    println!("{:?}", association);
    println!("{:?}", server_handle);
    
    // Verify association properties
    assert_eq!(association.peer_ae_title(), "TLS-SCP");
    assert!(!association.presentation_contexts().is_empty());
    
    // Release the association
    association.release().expect("Failed to release association");
    
    // Wait for server to complete
    server_handle.join().expect("Server thread failed");
}

#[cfg(all(feature = "async", feature = "tls"))]
#[tokio::test(flavor = "multi_thread")]
async fn test_tls_connection_async() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let server_addr = listener.local_addr()?;
    
    // Server configuration
    let (server_tls_config, client_tls_config) = create_test_config().expect("Failed to create test config");
    let server_options = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title("ASYNC-TLS-SCP")
        .with_abstract_syntax(VERIFICATION)
        .tls_config((*server_tls_config).clone());
    
    // Spawn server task
    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)?;
        let mut association = server_options.establish_tls_async(stream).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)?;
        
        // Verify we can access association properties
        assert_eq!(association.peer_ae_title(), "ASYNC-TLS-SCU");
        assert!(!association.presentation_contexts().is_empty());
        
        // Wait for a release request
        let pdu = association.receive().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)?;
        if let dicom_ul::Pdu::ReleaseRQ = pdu {
            association.send(&dicom_ul::Pdu::ReleaseRP).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)?;
        }
        
        Ok::<(), Box<dyn std::error::Error + Send + Sync + 'static>>(())
    });
    
    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    
    // Client configuration
    let client_options = ClientAssociationOptions::new()
        .calling_ae_title("ASYNC-TLS-SCU")
        .called_ae_title("ASYNC-TLS-SCP")
        .with_abstract_syntax(VERIFICATION)
        .server_name("localhost")
        .tls_config((*client_tls_config).clone());
    
    // Establish TLS connection
    let association = client_options.establish_tls_async(server_addr).await?;
    
    // Verify association properties
    assert_eq!(association.peer_ae_title(), "ASYNC-TLS-SCP");
    assert!(!association.presentation_contexts().is_empty());
    
    // Release the association
    association.release().await?;
    
    // Wait for server to complete
    server_handle.await??;
    
    Ok(())
}

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
