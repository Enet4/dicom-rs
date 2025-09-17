use clap::Args;
use rustls::{ClientConfig, ServerConfig, SupportedProtocolVersion, pki_types::{CertificateDer, CertificateRevocationListDer, PrivateKeyDer, pem::PemObject}, server::WebPkiClientVerifier};
use tracing::{debug, info};
use std::{path::PathBuf, sync::Arc};
use snafu::{Snafu, ResultExt};

#[derive(Snafu, Debug)]
pub enum TLSError {
    #[snafu(display("IO error: {}", source))]
    Io { source: std::io::Error },
    #[snafu(display("PEM parse error: {}, path: {:?}", source, path))]
    PemParse { 
        source: rustls::pki_types::pem::Error,
        path: Option<PathBuf>,
     },
    #[snafu(display("Rustls error: {}", source))]
    Rustls { source: rustls::Error },

    #[snafu(display("Certificate verifier error: {}", source))]
    CertificateVerifier { source: rustls::client::VerifierBuilderError},

    #[snafu(display("Config error: {}", message))]
    Config { message: String },
}


#[derive(Args, Debug)]
pub struct TLSOptions {
    /// Enables mTLS (TLS for DICOM connections)
    #[arg(long = "tls", action = clap::ArgAction::SetTrue)]
    pub enabled: Option<bool>,

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

    #[arg(long, action = clap::ArgAction::SetFalse)]
    /// Load certitificates from the system root store
    pub system_roots: bool,

    /// How to handle peer certificates
    #[arg(long, value_enum, value_name = "opt", default_value_t = PeerCertOption::Require)]
    pub peer_cert: PeerCertOption,

    /// Allow unauthenticated clients (only valid for server)
    #[arg(long)]
    pub allow_unauthenticated: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum CryptoProvider {
    AwsLC,
    //RING
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum TLSProtocolVersion {
    TLS1_2,
    TLS1_3,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum PeerCertOption {
    /// Require the peer to present a valid certificate
    Require,
    /// Do not verify the peer certificate
    Ignore,
}

pub fn show_cipher_suites() -> ! {
    let provider = rustls::crypto::CryptoProvider::get_default().expect("No default crypto provider found");
    println!("Supported cipher suites: ");
    for suite in &provider.cipher_suites {
        println!("{:?}", suite.suite());
    }
    std::process::exit(0)
}

impl TLSOptions{
    /// Build a root cert store from system roots and any additional certs
    fn root_cert_store(&self) -> Result<rustls::RootCertStore, TLSError> {
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
    ///
    /// Lifetime: Lifetime of the struct is different than the returned references
    fn certs<'a>(&'a self) -> Result<Option<Vec<CertificateDer<'static>>>, TLSError> {
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
    fn crls<'a>(&'a self) -> Result<Option<Vec<CertificateRevocationListDer<'static>>>, TLSError> {
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
    /// 
    /// Lifetime: Lifetime of the struct is different than the returned references
    fn protocol_versions<'a>(&'a self) -> Vec<&'static SupportedProtocolVersion> {
        self.protocol_versions.iter().map(|v| match v {
            TLSProtocolVersion::TLS1_2 => &rustls::version::TLS12,
            TLSProtocolVersion::TLS1_3 => &rustls::version::TLS13
        }).collect()
    }

    /// Consume the options to create a client config
    pub fn client_config(&self) -> Result<ClientConfig, TLSError> {
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
                ConfigSnafu{ message: "Certificate provided but no private key"}.fail()
            }
            (None, _) => {
                let config = builder.with_no_client_auth();
                info!("Using client without certificate authentication");
                Ok(config)
            }
        }
    }

    /// Consume the options to create a server config
    pub fn server_config(&self) -> Result<ServerConfig, TLSError> {
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
            if self.allow_unauthenticated {
                info!("Allowing unauthenticated clients");
                cert_verifier = cert_verifier.allow_unauthenticated();
            }
            let cert_verifier = cert_verifier.build()
                .context(CertificateVerifierSnafu)?;
            builder.with_client_cert_verifier(cert_verifier)
        };
        if let (Some(certs), Some(key)) = (self.certs()?, &self.key) {
            let key = PrivateKeyDer::from_pem_file(key)
                .with_context(|_| PemParseSnafu{path: key.clone()})?;
            let config = builder.with_single_cert(certs, key)
                .context(RustlsSnafu)?;
            return Ok(config)
        }
        ConfigSnafu{ message: "Server requires both certificate and private key"}.fail()
    }

}
