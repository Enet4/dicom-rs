use clap::{App, Arg};
use dicom_ul::pdu::reader::{read_pdu, DEFAULT_MAX_PDU};
use dicom_ul::pdu::writer::write_pdu;
use dicom_ul::pdu::Pdu;
use snafu::{Backtrace, ErrorCompat, OptionExt, ResultExt, Snafu};
use std::io::Write;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[non_exhaustive]
enum Error {
    #[snafu(display("Could not clone socket"))]
    CloneSocket {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not send message"))]
    SendMessage {
        backtrace: Backtrace,
        source: std::sync::mpsc::SendError<ThreadMessage>,
    },
    #[snafu(display("Could not receive message"))]
    ReceiveMessage {
        backtrace: Backtrace,
        source: std::sync::mpsc::RecvError,
    },
    #[snafu(display("Could not close socket"))]
    CloseSocket {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not connect to destination SCP"))]
    Connect {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("SCP reader thread panicked"))]
    ScpReaderPanic { backtrace: Backtrace },
    #[snafu(display("SCU reader thread panicked"))]
    ScuReaderPanic { backtrace: Backtrace },
}

fn report<E: 'static>(err: E)
where
    E: std::error::Error,
    E: ErrorCompat,
{
    eprintln!("[ERROR] {}", err);
    if let Some(source) = err.source() {
        eprintln!();
        eprintln!("Caused by:");
        for (i, e) in std::iter::successors(Some(source), |e| e.source()).enumerate() {
            eprintln!("   {}: {}", i, e);
        }
    }

    let env_backtrace = std::env::var("RUST_BACKTRACE").unwrap_or_default();
    let env_lib_backtrace = std::env::var("RUST_LIB_BACKTRACE").unwrap_or_default();
    if env_lib_backtrace == "1" || (env_backtrace == "1" && env_lib_backtrace != "0") {
        if let Some(backtrace) = ErrorCompat::backtrace(&err) {
            eprintln!();
            eprintln!("Backtrace:");
            eprintln!("{}", backtrace);
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ProviderType {
    /// Service class provider
    Scp,
    /// Service class user
    Scu,
}

#[derive(Debug)]
pub enum ThreadMessage {
    SendPdu {
        to: ProviderType,
        pdu: Pdu,
    },
    ReadErr {
        from: ProviderType,
        err: dicom_ul::pdu::reader::Error,
    },
    WriteErr {
        from: ProviderType,
        err: dicom_ul::pdu::writer::Error,
    },
    Shutdown {
        initiator: ProviderType,
    },
}

fn run(
    scu_stream: &mut TcpStream,
    destination_addr: &str,
    strict: bool,
    verbose: bool,
    max_pdu_length: u32,
) -> Result<()> {
    // Before we do anything, let's also open another connection to the destination
    // SCP.
    match TcpStream::connect(destination_addr) {
        Ok(ref mut scp_stream) => {
            let (message_tx, message_rx): (Sender<ThreadMessage>, Receiver<ThreadMessage>) =
                mpsc::channel();

            let scu_reader_thread: JoinHandle<Result<()>>;
            let scp_reader_thread: JoinHandle<Result<()>>;

            {
                let mut reader = scu_stream.try_clone().context(CloneSocket)?;
                let message_tx = message_tx.clone();
                scu_reader_thread = thread::spawn(move || {
                    loop {
                        match read_pdu(&mut reader, max_pdu_length, strict) {
                            Ok(pdu) => {
                                message_tx
                                    .send(ThreadMessage::SendPdu {
                                        to: ProviderType::Scp,
                                        pdu,
                                    })
                                    .context(SendMessage)?;
                            }
                            Err(dicom_ul::pdu::reader::Error::NoPduAvailable { .. }) => {
                                message_tx
                                    .send(ThreadMessage::Shutdown {
                                        initiator: ProviderType::Scu,
                                    })
                                    .context(SendMessage)?;
                                break;
                            }
                            Err(err) => {
                                message_tx
                                    .send(ThreadMessage::ReadErr {
                                        from: ProviderType::Scu,
                                        err,
                                    })
                                    .context(SendMessage)?;
                                break;
                            }
                        }
                    }

                    Ok(())
                });
            }

            {
                let mut reader = scp_stream.try_clone().context(CloneSocket)?;
                scp_reader_thread = thread::spawn(move || {
                    loop {
                        match read_pdu(&mut reader, max_pdu_length, strict) {
                            Ok(pdu) => {
                                message_tx
                                    .send(ThreadMessage::SendPdu {
                                        to: ProviderType::Scu,
                                        pdu,
                                    })
                                    .context(SendMessage)?;
                            }
                            Err(dicom_ul::pdu::reader::Error::NoPduAvailable { .. }) => {
                                message_tx
                                    .send(ThreadMessage::Shutdown {
                                        initiator: ProviderType::Scp,
                                    })
                                    .context(SendMessage)?;
                                break;
                            }
                            Err(err) => {
                                message_tx
                                    .send(ThreadMessage::ReadErr {
                                        from: ProviderType::Scp,
                                        err,
                                    })
                                    .context(SendMessage)?;
                                break;
                            }
                        }
                    }

                    Ok(())
                });
            }
            let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);

            loop {
                let message = message_rx.recv().context(ReceiveMessage)?;
                match message {
                    ThreadMessage::SendPdu { to, pdu } => match to {
                        ProviderType::Scu => {
                            if verbose {
                                println!("scu <---- scp: {}", pdu.short_description());
                            }
                            buffer.clear();
                            write_pdu(&mut buffer, &pdu).unwrap();
                            scu_stream.write_all(&buffer).unwrap();
                        }
                        ProviderType::Scp => {
                            if verbose {
                                println!("scu ----> scp: {}", pdu.short_description());
                            }
                            buffer.clear();
                            write_pdu(&mut buffer, &pdu).unwrap();
                            scp_stream.write_all(&buffer).unwrap();
                        }
                    },
                    ThreadMessage::ReadErr { from, err } => {
                        eprintln!("error reading from {:?}:", from);
                        report(err);
                        break;
                    }
                    ThreadMessage::WriteErr { from, err } => {
                        eprintln!("error writing to {:?}", from);
                        report(err);
                        break;
                    }
                    ThreadMessage::Shutdown { initiator } => {
                        if verbose {
                            println!("shutdown initiated from: {:?}", initiator);
                        }
                        break;
                    }
                }
            }

            scu_stream.shutdown(Shutdown::Read).context(CloseSocket)?;
            scu_reader_thread.join().ok().context(ScuReaderPanic)??;

            scp_stream.shutdown(Shutdown::Read).context(CloseSocket)?;
            scp_reader_thread.join().ok().context(ScpReaderPanic)??;

            Ok(())
        }
        Err(e) => {
            scu_stream.shutdown(Shutdown::Both).context(CloseSocket)?;
            Err(e).context(Connect)
        }
    }
}

fn main() {
    let default_max = DEFAULT_MAX_PDU.to_string();
    let matches = App::new("scpproxy")
        .arg(
            Arg::with_name("destination-host")
                .help("The destination host name (SCP)")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("destination-port")
                .help("The destination host port (SCP)")
                .required(true)
                .index(2),
        )
        .arg(
            Arg::with_name("listen-port")
                .help("The port that we will listen for SCU connections on")
                .short("-lp")
                .long("--listen-port")
                .default_value("3333")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("strict")
                .help("Enforce max PDU length")
                .short("-s")
                .long("--strict")
                .required(false)
                .takes_value(false),
        )
        .arg(
            Arg::with_name("verbose")
                .help("Verbose")
                .short("-v")
                .long("--verbose")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("max-pdu-length")
                .help("Maximum PDU length")
                .short("-m")
                .long("--max-pdu-length")
                .default_value(&default_max)
                .takes_value(true),
        )
        .get_matches();

    let destination_host = matches.value_of("destination-host").unwrap();
    let destination_port = matches.value_of("destination-port").unwrap();
    let listen_port = matches.value_of("listen-port").unwrap();
    let strict: bool = matches.is_present("strict");
    let verbose = matches.is_present("verbose");
    let max_pdu_length: u32 = matches.value_of("max-pdu-length").unwrap().parse().unwrap();

    let listen_addr = format!("0.0.0.0:{}", listen_port);
    let destination_addr = format!("{}:{}", destination_host, destination_port);

    let listener = TcpListener::bind(&listen_addr).unwrap();
    if verbose {
        println!("listening on: {}", listen_addr);
        println!("forwarding to: {}", destination_addr);
    }
    for mut stream in listener.incoming() {
        match stream {
            Ok(ref mut scu_stream) => {
                if let Err(e) = run(
                    scu_stream,
                    &destination_addr,
                    strict,
                    verbose,
                    max_pdu_length,
                ) {
                    report(e);
                }
            }
            Err(e) => {
                eprintln!("[ERROR] {}", e);
            }
        }
    }
}
