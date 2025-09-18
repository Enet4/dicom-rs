# DICOM-rs `storescu`

[![CratesIO](https://img.shields.io/crates/v/dicom-storescu.svg)](https://crates.io/crates/dicom-storescu)
[![Documentation](https://docs.rs/dicom-storescu/badge.svg)](https://docs.rs/dicom-storescu)

This is an implementation of the DICOM Storage SCU (C-STORE),
which can be used for uploading DICOM files to other DICOM devices.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

Note that this tool is not necessarily a drop-in replacement
for `storescu` tools in other DICOM software projects.

```none
DICOM C-STORE SCU

Usage: dicom-storescu [OPTIONS] <ADDR> <FILES>...

Arguments:
  <ADDR>      socket address to Store SCP, optionally with AE title (example: "STORE-SCP@127.0.0.1:104")
  <FILES>...  the DICOM file(s) to store

Options:
  -v, --verbose                                            verbose mode
      --calling-ae-title <CALLING_AE_TITLE>                the calling Application Entity title [default: STORE-SCU]
      --called-ae-title <CALLED_AE_TITLE>                  the called Application Entity title, overrides AE title in address if present [default: ANY-SCP]
      --max-pdu-length <MAX_PDU_LENGTH>                    the maximum PDU length accepted by the SCU [default: 16384]
      --fail-first                                         fail if not all DICOM files can be transferred
      --never-transcode                                    fail file transfer if it cannot be done without transcoding
      --username <USERNAME>                                User Identity username
      --password <PASSWORD>                                User Identity password
      --kerberos-service-ticket <KERBEROS_SERVICE_TICKET>  User Identity Kerberos service ticket
      --saml-assertion <SAML_ASSERTION>                    User Identity SAML assertion
      --jwt <JWT>                                          User Identity JWT
  -c, --concurrency <CONCURRENCY>                          Dispatch these many service users to send files in parallel
  -h, --help                                               Print help (see more with '--help')
  -V, --version                                            Print version

TLS Options:
      --tls                                Enables mTLS (TLS for DICOM connections)
      --crypto-provider <provider>         Crypto provider to use, see documentation (https://docs.rs/rustls/latest/rustls/index.html) for details [default: aws-lc] [possible values:
                                           aws-lc]
      --cipher-suites <cipher1,...>        List of cipher suites to use. If not specified, the default cipher suites for the selected crypto provider will be used
      --protocol-versions <version,...>    TLS protocol versions to enable [default: tls1-2 tls1-3] [possible values: tls1-2, tls1-3]
      --key </path/to/key.pem,...>         Path to private key file in PEM format
      --cert </path/to/cert.pem,...>       Path to certificate file in PEM format
      --add-certs </path/to/cert.pem,...>  Path to additional CA certificates (comma separated) in PEM format to add to the root store
      --add-crls </path/to/crl.pem,...>    Add Certificate Revocation Lists (CRLs) to the server's certificate verifier
      --system-roots                       Load certitificates from the system root store
      --peer-cert <opt>                    How to handle peer certificates [default: require] [possible values: require, ignore]
      --allow-unauthenticated              Allow unauthenticated clients (only valid for server)
dd
```

## Examples

### Send two files to remote

```sh
dicom-storescu MAIN-STORAGE@192.168.1.99:104 xray1.dcm xray2.dcm
```

### Use a TLS connection

The following example assumes you have a TLS enabled dicom server running on the destination server.

The destination will need to be configured for the dicom modality as well as the cert of this client.

If the server has a self-signed cert, make sure the CA that signed it is passed in via `add-certs`

```sh
dicom-storescu --tls \
    --calling-ae-title TLS-CLIENT \
    --cert /opt/client.pem \
    --key /opt/client.key.pem \
    --add-certs /opt/ca.pem \
    MAIN-STORAGE@192.168.1.99:104 xray1.dcm xray2.dcm
```

### Use an anonymous TLS connection

The following example assumes you have a TLS enabled dicom server running on the destination server.

The destination will need to be configured for the dicom modality and allow anonymous TLS

If the server has a self-signed cert, make sure the CA that signed it is passed in via `add-certs`

```sh
dicom-storescu --tls \
    --calling-ae-title TLS-CLIENT \
    --add-certs /opt/ca.pem \
    MAIN-STORAGE@192.168.1.99:104 xray1.dcm xray2.dcm
```