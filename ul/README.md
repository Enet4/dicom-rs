# DICOM-rs `ul`

[![CratesIO](https://img.shields.io/crates/v/dicom-ul.svg)](https://crates.io/crates/dicom-ul)
[![Documentation](https://docs.rs/dicom-ul/badge.svg)](https://docs.rs/dicom-ul)

This is an implementation of the DICOM upper layer protocol.

## Testing

### TLS

TLS testing requires a Certificate authority, and signed client/server key pairs

A function is provided within `tests/association.rs` which:

1. Creates a CA with a `US` country code
2. Creates certificate signing requests for two "clients" (one client and one server)
3. Signs the certificates using the CA

When finished, there should be a `.pem`, and a `.key.pem` for the client, server and CA client and server,

If you live outside of the US or want to change the IP/DNS of the configured
client/server

Change the `country_name` and/or `organization_name` in the test code.