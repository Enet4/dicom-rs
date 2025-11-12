use clap::Args;
use rustls::{ClientConfig, ServerConfig, SupportedProtocolVersion, pki_types::{CertificateDer, CertificateRevocationListDer, PrivateKeyDer, pem::PemObject}, server::WebPkiClientVerifier};
use tracing::{debug, info};
use std::{path::PathBuf, sync::Arc};
use snafu::{Snafu, ResultExt};

#[derive(Snafu, Debug)]
pub enum MissingPemObject {
    #[snafu(display("Missing Certificate"))]
    Certificate,
    #[snafu(display("Missing Private Key"))]
    PrivateKey,
}

#[derive(Snafu, Debug)]
pub enum TlsError {
    #[snafu(display("IO error"))] 
    Io { source: std::io::Error },
    #[snafu(display("PEM parse error in path: {}", path.as_ref().map(|p| p.display().to_string()).unwrap_or("unknown".into())))]
    PemParse { 
        source: rustls::pki_types::pem::Error,
        path: Option<PathBuf>,
     },
    #[snafu(display("Rustls error"))]
    Rustls { source: rustls::Error },

    #[snafu(display("Certificate verifier error"))]
    CertificateVerifier { source: rustls::client::VerifierBuilderError},

    #[snafu(display("Config error: {missing}"))]
    Config { missing: MissingPemObject },
}


#[derive(Args, Debug)]
pub struct TlsOptions {
    /// Enables mTLS (TLS for DICOM connections)
    #[arg(long = "tls", default_value = "false")]
    pub enabled: bool,

    /// Crypto provider to use, see documentation (https://docs.rs/rustls/latest/rustls/index.html) for details
    #[arg(long, value_enum, default_value_t = CryptoProvider::AwsLC, value_name = "provider")]
    pub crypto_provider: CryptoProvider,

    /// List of cipher suites to use. If not specified, the default cipher suites for the selected crypto provider will be used.
    #[arg(long, value_name = "cipher1,...")]
    pub cipher_suites: Option<Vec<String>>,

    /// TLS protocol versions to enable
    #[arg(long, value_enum, value_name = "version,...", default_values_t = vec![TLSProtocolVersion::TLS1_2, TLSProtocolVersion::TLS1_3])]
    pub protocol_versions: Vec<TLSProtocolVersion>,

    /// Path to private key file in PEM format
    #[arg(long, value_name = "/path/to/key.pem,...")]
    pub key: Option<PathBuf>,

    /// Path to certificate file in PEM format
    #[arg(long, value_name = "/path/to/cert.pem,...")]
    pub cert: Option<PathBuf>,

    /// Path to additional CA certificates (comma separated) in PEM format to add to the root store
    #[arg(long, value_name = "/path/to/cert.pem,...")]
    pub add_certs: Option<Vec<PathBuf>>,

    /// Add Certificate Revocation Lists (CRLs) to the server's certificate verifier
    #[arg(long, value_name = "/path/to/crl.pem,...")]
    pub add_crls: Option<Vec<PathBuf>>,

    /// Load certitificates from the system root store
    #[arg(long, action = clap::ArgAction::SetFalse)]
    pub system_roots: bool,

    /// How to handle peer certificates
    #[arg(long, value_enum, value_name = "opt", default_value_t = PeerCertOption::Require)]
    pub peer_cert: PeerCertOption,

}

#[derive(Args, Debug)]
pub struct TlsAcceptorOptions {
    /// Allow unauthenticated clients (only valid for server)
    #[arg(long)]
    pub allow_unauthenticated: bool,
}

/// Crypto provider options
/// 
/// See rustls 
/// [Cryptograpy providers](https://docs.rs/rustls/latest/rustls/#cryptography-providers)
/// for more details
/// 
/// Currently only AWS-LC is supported
#[non_exhaustive]
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum CryptoProvider {
    AwsLC,
    //RING
}

/// TLS protocol version options
/// 
/// Subset of rustls 
/// [ProtocolVersions](https://docs.rs/rustls/latest/rustls/enum.ProtocolVersion.html#variants) 
/// supported
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum TLSProtocolVersion {
    TLS1_2,
    TLS1_3,
}

/// Peer certificate handling options
/// 
/// Defines how the TLS connection should handle peer certificates
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum PeerCertOption {
    /// Require the peer to present a valid certificate
    Require,
    /// Do not verify the peer certificate
    Ignore,
}

/// Show the supported cipher suites for the default crypto provider
pub fn show_cipher_suites(){
    let provider = rustls::crypto::CryptoProvider::get_default().expect("No default crypto provider found");
    println!("Supported cipher suites: ");
    for suite in &provider.cipher_suites {
        println!("{:?}", suite.suite());
    }
}

impl TlsOptions{
    /// Build a root cert store from system roots and any additional certs
    fn root_cert_store(&self) -> Result<rustls::RootCertStore, TlsError> {
        let mut root_store = rustls::RootCertStore::empty();
        // Load system roots unless disabled
        if self.system_roots{
            let system_roots = rustls_native_certs::load_native_certs();
            root_store.add_parsable_certificates(system_roots.certs);
        }
        // Add any extra certs
        if let Some(certs) = &self.add_certs{
            let mut loaded_certs = Vec::new();
            for path in certs {
                let cert = CertificateDer::from_pem_file(path)
                    .with_context(|_| PemParseSnafu{path: path.clone()})?;
                loaded_certs.push(cert);
            }
            root_store.add_parsable_certificates(loaded_certs);
        }
        Ok(root_store)
    }

    /// Load client certs if provided
    fn certs(&self) -> Result<Option<Vec<CertificateDer<'static>>>, TlsError> {
        // If a certificate is provided, load it as a cert chain
        match self.cert.as_ref() {
            Some(path) => {
                let certs = CertificateDer::pem_file_iter(path)
                    .with_context(|_| PemParseSnafu{path: path.clone()})?
                    .collect::<Result<Vec<_>, _>>()
                    .with_context(|_| PemParseSnafu{path: path.clone()})?;
                Ok(Some(certs))
            }
            None => Ok(None),
        }
    }

    /// Load CRLs if provided
    fn crls(&self) -> Result<Option<Vec<CertificateRevocationListDer<'static>>>, TlsError> {
        match self.add_crls.as_ref() {
            Some(crls) => {
                let mut loaded_crls = Vec::new();
                for path in crls {
                    let crl = CertificateRevocationListDer::from_pem_file(path)
                        .with_context(|_| PemParseSnafu{path: path.clone()})?;
                    loaded_crls.push(crl);
                }
                Ok(Some(loaded_crls))
            }
            None => Ok(None),
        }
    }

    /// Map selected protocol versions to rustls types
    fn protocol_versions(&self) -> Vec<&'static SupportedProtocolVersion> {
        self.protocol_versions.iter().map(|v| match v {
            TLSProtocolVersion::TLS1_2 => &rustls::version::TLS12,
            TLSProtocolVersion::TLS1_3 => &rustls::version::TLS13
        }).collect()
    }

    /// Consume the options to create a client config
    pub fn client_config(&self) -> Result<ClientConfig, TlsError> {
        debug!("Building client config with options: {:?}", self);
        // Get the crypto provider
        let provider = match self.crypto_provider {
            CryptoProvider::AwsLC => rustls::crypto::aws_lc_rs::default_provider(),
        };
        let builder = ClientConfig::builder_with_provider(provider.into())
            .with_protocol_versions(self.protocol_versions().as_slice())
            .context(RustlsSnafu)?
            .with_root_certificates(self.root_cert_store()?);
        match (self.certs()?, &self.key) {
            (Some(certs), Some(key)) => {
                info!("Using client certificate authentication");
                let key = PrivateKeyDer::from_pem_file(key)
                    .with_context(|_| PemParseSnafu{path: key.clone()})?;
                let config = builder.with_client_auth_cert(certs, key)
                    .context(RustlsSnafu)?;
                Ok(config)
            }
            (Some(_), None) => {
                ConfigSnafu{ missing: MissingPemObject::PrivateKey }.fail()
            }
            (None, _) => {
                let config = builder.with_no_client_auth();
                info!("Using client without certificate authentication");
                Ok(config)
            }
        }
    }

    /// Consume the options to create a server config
    pub fn server_config(&self, acceptor_options: &TlsAcceptorOptions) -> Result<ServerConfig, TlsError> {
        // Get the crypto provider
        let provider = match self.crypto_provider {
            CryptoProvider::AwsLC => Arc::new(rustls::crypto::aws_lc_rs::default_provider()),
        };
        let builder = ServerConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(self.protocol_versions().as_slice())
            .context(RustlsSnafu)?;
        let builder = if let PeerCertOption::Ignore = self.peer_cert {
            builder.with_no_client_auth()
        } else {
            let mut cert_verifier = WebPkiClientVerifier::builder_with_provider(
                self.root_cert_store()?.into(), provider
            );
            if let Some(crl_paths) = self.crls()? {
                cert_verifier = cert_verifier.with_crls(crl_paths);
            }
            if acceptor_options.allow_unauthenticated {
                info!("Allowing unauthenticated clients");
                cert_verifier = cert_verifier.allow_unauthenticated();
            }
            let cert_verifier = cert_verifier.build()
                .context(CertificateVerifierSnafu)?;
            builder.with_client_cert_verifier(cert_verifier)
        };
        match (self.certs()?, &self.key) {
            (Some(certs), Some(key)) => {
                let key = PrivateKeyDer::from_pem_file(key)
                    .with_context(|_| PemParseSnafu{path: key.clone()})?;
                let config = builder.with_single_cert(certs, key)
                    .context(RustlsSnafu)?;
                Ok(config)
            }
            (Some(_), None) => {
                ConfigSnafu{ missing: MissingPemObject::PrivateKey }.fail()
            }
            (None, _) => {
                ConfigSnafu{ missing: MissingPemObject::Certificate }.fail()
            }
        }
    }

}
