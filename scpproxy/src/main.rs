use clap::{App, Arg};
use dicom_ul::pdu::reader::{read_pdu, DEFAULT_MAX_PDU};
use dicom_ul::pdu::writer::write_pdu;
use dicom_ul::pdu::Pdu;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("Could not clone socket: {}", source))]
    CloneSocket {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not send message: {}", source))]
    SendMessage {
        backtrace: Backtrace,
        source: std::sync::mpsc::SendError<ThreadMessage>,
    },
    #[snafu(display("Could not receive message: {}", source))]
    ReceiveMessage {
        backtrace: Backtrace,
        source: std::sync::mpsc::RecvError,
    },
    #[snafu(display("Could not close socket: {}", source))]
    CloseSocket {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not connect to destination SCP: {}", source))]
    Connect {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("SCP reader thread panicked"))]
    ScpReaderPanic {
        backtrace: Backtrace,
    },
    #[snafu(display("SCU reader thread panicked"))]
    ScuReaderPanic {
        backtrace: Backtrace,
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

fn run(scu_stream: &mut TcpStream, destination_addr: &str) -> Result<()> {
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
                        match read_pdu(&mut reader, DEFAULT_MAX_PDU) {
                            Ok(pdu) => {
                                message_tx.send(ThreadMessage::SendPdu {
                                    to: ProviderType::Scp,
                                    pdu,
                                }).context(SendMessage)?;
                            }
                            Err(dicom_ul::pdu::reader::Error::NoPduAvailable {..}) => {
                                message_tx.send(ThreadMessage::Shutdown {
                                    initiator: ProviderType::Scu,
                                }).context(SendMessage)?;
                                break;
                            }
                            Err(err) => {
                                message_tx.send(ThreadMessage::ReadErr {
                                    from: ProviderType::Scu,
                                    err,
                                }).context(SendMessage)?;
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
                        match read_pdu(&mut reader, DEFAULT_MAX_PDU) {
                            Ok(pdu) => {
                                message_tx.send(ThreadMessage::SendPdu {
                                    to: ProviderType::Scu,
                                    pdu,
                                }).context(SendMessage)?;
                            }
                            Err(dicom_ul::pdu::reader::Error::NoPduAvailable { .. }) => {
                                    message_tx.send(ThreadMessage::Shutdown {
                                        initiator: ProviderType::Scp,
                                    }).context(SendMessage)?;
                                break;
                            }
                            Err(err) => {
                                message_tx.send(ThreadMessage::ReadErr {
                                    from: ProviderType::Scp,
                                    err,
                                }).context(SendMessage)?;
                                break;
                            }
                        }
                    }

                    Ok(())
                });
            }

            loop {
                let message = message_rx.recv().context(ReceiveMessage)?;
                match message {
                    ThreadMessage::SendPdu { to, pdu } => match to {
                        ProviderType::Scu => {
                            println!("scu <---- scp: {:?}", &pdu);
                            write_pdu(scu_stream, &pdu).unwrap();
                        }
                        ProviderType::Scp => {
                            println!("scu ----> scp: {:?}", &pdu);
                            write_pdu(scp_stream, &pdu).unwrap();
                        }
                    },
                    ThreadMessage::ReadErr { from, err } => {
                        eprintln!("error reading from {:?}: {}", from, err);
                        break;
                    }
                    ThreadMessage::WriteErr { from, err } => {
                        eprintln!("error writing to {:?}: {}", from, err);
                        break;
                    }
                    ThreadMessage::Shutdown { initiator } => {
                        println!("shutdown initiated from: {:?}", initiator);
                        break;
                    }
                }
            }

            scu_stream.shutdown(Shutdown::Read).context(CloseSocket)?;
            scu_reader_thread
                .join()
                .ok()
                .context(ScuReaderPanic)??;

            scp_stream.shutdown(Shutdown::Read).context(CloseSocket)?;
            scp_reader_thread
                .join()
                .ok()
                .context(ScpReaderPanic)??;

            Ok(())
        }
        Err(e) => {
            scu_stream.shutdown(Shutdown::Both).context(CloseSocket)?;
            Err(e).context(Connect)
        }
    }
}

fn main() {
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
        .get_matches();

    let destination_host = matches.value_of("destination-host").unwrap();
    let destination_port = matches.value_of("destination-port").unwrap();
    let listen_port = matches.value_of("listen-port").unwrap();

    let listen_addr = format!("0.0.0.0:{}", listen_port);
    let destination_addr = format!("{}:{}", destination_host, destination_port);

    let listener = TcpListener::bind(&listen_addr).unwrap();
    println!("listening on: {}", listen_addr);
    println!("forwarding to: {}", destination_addr);

    for mut stream in listener.incoming() {
        match stream {
            Ok(ref mut scu_stream) => {
                if let Err(e) = run(scu_stream, &destination_addr) {
                    eprintln!("error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("error: {}", e);
            }
        }
    }
}
