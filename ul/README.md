# DICOM-rs `ul`

[![CratesIO](https://img.shields.io/crates/v/dicom-ul.svg)](https://crates.io/crates/dicom-ul)
[![Documentation](https://docs.rs/dicom-ul/badge.svg)](https://docs.rs/dicom-ul)

This is an implementation of the DICOM upper layer protocol.

## Testing

### TLS

TLS testing requires a Certificate authority, and signed client/server key pairs

A bash script is provided in `assets/generate_certs.sh` using `openssl` which:

1. Creates a CA with a `US` country code
2. Creates certificate signing requests for two "clients" (one client and one server)
   using the configuration in `openssl.cnf`
3. Signs the certificates using the CA

When finished, there should be a `.crt`, a `.key` and a `.csr` for both the
client and server,
there will also be a `.srl` for the CA which contains the last key's serial number

If you live outside of the US or want to change the IP/DNS of the configured
client/server

1. Change the `C` and `CN` in line 3 of the bash script if you are in a
different country
1. Change the entries in `openssl.cnf` for your domain/IP if needed (`C`, `CN`
and `DNS.1` and `IP.1`)
