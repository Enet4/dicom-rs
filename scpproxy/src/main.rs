use clap::{Arg, Command};
use dicom_ul::pdu::reader::{read_pdu, DEFAULT_MAX_PDU};
use dicom_ul::pdu::writer::write_pdu;
use dicom_ul::pdu::Pdu;
use snafu::{Backtrace, OptionExt, Report, ResultExt, Snafu, Whatever};
use std::io::Write;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use tracing::error;

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
                let mut reader = scu_stream.try_clone().context(CloneSocketSnafu)?;
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
                                    .context(SendMessageSnafu)?;
                            }
                            Err(dicom_ul::pdu::reader::Error::NoPduAvailable { .. }) => {
                                message_tx
                                    .send(ThreadMessage::Shutdown {
                                        initiator: ProviderType::Scu,
                                    })
                                    .context(SendMessageSnafu)?;
                                break;
                            }
                            Err(err) => {
                                message_tx
                                    .send(ThreadMessage::ReadErr {
                                        from: ProviderType::Scu,
                                        err,
                                    })
                                    .context(SendMessageSnafu)?;
                                break;
                            }
                        }
                    }

                    Ok(())
                });
            }

            {
                let mut reader = scp_stream.try_clone().context(CloneSocketSnafu)?;
                scp_reader_thread = thread::spawn(move || {
                    loop {
                        match read_pdu(&mut reader, max_pdu_length, strict) {
                            Ok(pdu) => {
                                message_tx
                                    .send(ThreadMessage::SendPdu {
                                        to: ProviderType::Scu,
                                        pdu,
                                    })
                                    .context(SendMessageSnafu)?;
                            }
                            Err(dicom_ul::pdu::reader::Error::NoPduAvailable { .. }) => {
                                message_tx
                                    .send(ThreadMessage::Shutdown {
                                        initiator: ProviderType::Scp,
                                    })
                                    .context(SendMessageSnafu)?;
                                break;
                            }
                            Err(err) => {
                                message_tx
                                    .send(ThreadMessage::ReadErr {
                                        from: ProviderType::Scp,
                                        err,
                                    })
                                    .context(SendMessageSnafu)?;
                                break;
                            }
                        }
                    }

                    Ok(())
                });
            }
            let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);

            loop {
                let message = message_rx.recv().context(ReceiveMessageSnafu)?;
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
                        error!("error reading from {:?}: {}", from, Report::from_error(err));
                        break;
                    }
                    ThreadMessage::WriteErr { from, err } => {
                        error!("error writing to {:?}: {}", from, Report::from_error(err));
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

            scu_stream
                .shutdown(Shutdown::Read)
                .context(CloseSocketSnafu)?;
            scu_reader_thread
                .join()
                .ok()
                .context(ScuReaderPanicSnafu)??;

            scp_stream
                .shutdown(Shutdown::Read)
                .context(CloseSocketSnafu)?;
            scp_reader_thread
                .join()
                .ok()
                .context(ScpReaderPanicSnafu)??;

            Ok(())
        }
        Err(e) => {
            scu_stream
                .shutdown(Shutdown::Both)
                .context(CloseSocketSnafu)?;
            Err(e).context(ConnectSnafu)
        }
    }
}

fn main() {
    tracing::subscriber::set_global_default(tracing_subscriber::FmtSubscriber::new())
        .whatever_context("Could not set up global tracing subscriber")
        .unwrap_or_else(|e: snafu::Whatever| {
            eprintln!("[ERROR] {}", Report::from_error(e));
        });

    let default_max = DEFAULT_MAX_PDU.to_string();
    let matches = Command::new("scpproxy")
        .arg(
            Arg::new("destination-host")
                .help("The destination host name (SCP)")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("destination-port")
                .help("The destination host port (SCP)")
                .required(true)
                .index(2),
        )
        .arg(
            Arg::new("listen-port")
                .help("The port that we will listen for SCU connections on")
                .short('l')
                .long("--listen-port")
                .default_value("3333")
                .takes_value(true),
        )
        .arg(
            Arg::new("strict")
                .help("Enforce max PDU length")
                .short('s')
                .long("--strict")
                .required(false)
                .takes_value(false),
        )
        .arg(
            Arg::new("verbose")
                .help("Verbose")
                .short('v')
                .long("--verbose")
                .takes_value(false),
        )
        .arg(
            Arg::new("max-pdu-length")
                .help("Maximum PDU length")
                .short('m')
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
                    error!("{}", Report::from_error(e));
                }
            }
            r @ Err(_) => {
                let e: Whatever = r
                    .whatever_context("Could not obtain TCP stream")
                    .unwrap_err();
                error!("{}", Report::from_error(e));
            }
        }
    }
}
