use bytes::BytesMut;
use clap::{crate_version, value_parser, Arg, ArgAction, Command};
use dicom_ul::association::read_pdu_from_wire;
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
        #[snafu(source(from(std::sync::mpsc::SendError<ThreadMessage>, Box::from)))]
        source: Box<std::sync::mpsc::SendError<ThreadMessage>>,
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
        err: dicom_ul::association::Error,
    },
    WriteErr {
        from: ProviderType,
        err: dicom_ul::pdu::WriteError,
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
                let mut buf = BytesMut::with_capacity(max_pdu_length as usize);
                let message_tx = message_tx.clone();
                scu_reader_thread = thread::spawn(move || {
                    loop {
                        match read_pdu_from_wire(&mut reader, &mut buf, max_pdu_length, strict) {
                            Ok(pdu) => {
                                message_tx
                                    .send(ThreadMessage::SendPdu {
                                        to: ProviderType::Scp,
                                        pdu,
                                    })
                                    .context(SendMessageSnafu)?;
                            }
                            Err(dicom_ul::association::Error::ConnectionClosed) => {
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
                let mut buf = BytesMut::with_capacity(max_pdu_length as usize);
                scp_reader_thread = thread::spawn(move || {
                    loop {
                        match read_pdu_from_wire(&mut reader, &mut buf, max_pdu_length, strict) {
                            Ok(pdu) => {
                                message_tx
                                    .send(ThreadMessage::SendPdu {
                                        to: ProviderType::Scu,
                                        pdu,
                                    })
                                    .context(SendMessageSnafu)?;
                            }
                            Err(dicom_ul::association::Error::ConnectionClosed) => {
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
                            println!("shutdown initiated from: {initiator:?}");
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

fn command() -> Command {
    Command::new("dicom-scpproxy")
        .version(crate_version!())
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
                .long("listen-port")
                .value_parser(value_parser!(u16).range(1..))
                .default_value("3333"),
        )
        .arg(
            Arg::new("strict")
                .help("Enforce max PDU length")
                .short('s')
                .long("strict")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .help("Verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("max-pdu-length")
                .help("Maximum PDU length")
                .short('m')
                .long("max-pdu-length")
                .value_parser(value_parser!(u32).range(4096..=131_072))
                .default_value("16384"),
        )
}

fn main() {
    tracing::subscriber::set_global_default(tracing_subscriber::FmtSubscriber::new())
        .whatever_context("Could not set up global tracing subscriber")
        .unwrap_or_else(|e: snafu::Whatever| {
            eprintln!("[ERROR] {}", Report::from_error(e));
        });

    let matches = command().get_matches();

    let destination_host = matches.get_one::<String>("destination-host").unwrap();
    let destination_port = matches.get_one::<String>("destination-port").unwrap();
    let listen_port: u16 = *matches.get_one("listen-port").unwrap();
    let strict: bool = matches.get_flag("strict");
    let verbose = matches.get_flag("verbose");
    let max_pdu_length: u32 = *matches.get_one("max-pdu-length").unwrap();

    let listen_addr = format!("0.0.0.0:{listen_port}");
    let destination_addr = format!("{destination_host}:{destination_port}");

    let listener = TcpListener::bind(&listen_addr).unwrap();
    if verbose {
        println!("listening on: {listen_addr}");
        println!("forwarding to: {destination_addr}");
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

#[cfg(test)]
mod tests {
    use super::command;

    #[test]
    fn verify_cli() {
        command().debug_assert();
    }
}
