extern crate clap;
use clap::{App, Arg};
use dicom_ul::pdu::reader::{read_pdu, DEFAULT_MAX_PDU};
use dicom_ul::pdu::writer::write_pdu;
use dicom_ul::pdu::PDU;
use quick_error::quick_error;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;

type Result<T> = std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: std::io::Error) {
            from()
        }
        DUL(err: dicom_ul::error::Error) {
            from()
        }
        ThreadPanicked {
            from()
        }
        RecvError(err: std::sync::mpsc::RecvError) {
            from()
        }
        SendError(err: std::sync::mpsc::SendError<ThreadMessage>) {
            from()
        }
    }
}

#[derive(Debug)]
pub enum ProviderType {
    SCP,
    SCU,
}

#[derive(Debug)]
pub enum ThreadMessage {
    SendPDU {
        to: ProviderType,
        pdu: PDU,
    },
    Err {
        from: ProviderType,
        err: dicom_ul::error::Error,
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
                let mut reader = scu_stream.try_clone()?;
                let message_tx = message_tx.clone();
                scu_reader_thread = thread::spawn(move || {
                    loop {
                        match read_pdu(&mut reader, DEFAULT_MAX_PDU) {
                            Ok(pdu) => {
                                message_tx.send(ThreadMessage::SendPDU {
                                    to: ProviderType::SCP,
                                    pdu,
                                })?;
                            }
                            Err(e) => {
                                if let dicom_ul::error::Error::NoPduAvailable = e {
                                    message_tx.send(ThreadMessage::Shutdown {
                                        initiator: ProviderType::SCU,
                                    })?;
                                } else {
                                    message_tx.send(ThreadMessage::Err {
                                        from: ProviderType::SCU,
                                        err: e,
                                    })?;
                                }
                                break;
                            }
                        }
                    }

                    Ok(())
                });
            }

            {
                let mut reader = scp_stream.try_clone()?;
                let message_tx = message_tx.clone();
                scp_reader_thread = thread::spawn(move || {
                    loop {
                        match read_pdu(&mut reader, DEFAULT_MAX_PDU) {
                            Ok(pdu) => {
                                message_tx.send(ThreadMessage::SendPDU {
                                    to: ProviderType::SCU,
                                    pdu,
                                })?;
                            }
                            Err(e) => {
                                if let dicom_ul::error::Error::NoPduAvailable = e {
                                    message_tx.send(ThreadMessage::Shutdown {
                                        initiator: ProviderType::SCP,
                                    })?;
                                } else {
                                    message_tx.send(ThreadMessage::Err {
                                        from: ProviderType::SCP,
                                        err: e,
                                    })?;
                                }
                                break;
                            }
                        }
                    }

                    Ok(())
                });
            }

            loop {
                let message = message_rx.recv()?;
                match message {
                    ThreadMessage::SendPDU { to, pdu } => match to {
                        ProviderType::SCU => {
                            println!("scu <---- scp: {:?}", &pdu);
                            write_pdu(scu_stream, &pdu).unwrap();
                        }
                        ProviderType::SCP => {
                            println!("scu ----> scp: {:?}", &pdu);
                            write_pdu(scp_stream, &pdu).unwrap();
                        }
                    },
                    ThreadMessage::Err { from, err } => {
                        println!("error from {:?}: {}", from, err);
                        break;
                    }
                    ThreadMessage::Shutdown { initiator } => {
                        println!("shutdown initiated from: {:?}", initiator);
                        break;
                    }
                }
            }

            scu_stream.shutdown(Shutdown::Read)?;
            scu_reader_thread
                .join()
                .map_err(|_| Error::ThreadPanicked)??;

            scp_stream.shutdown(Shutdown::Read)?;
            scp_reader_thread
                .join()
                .map_err(|_| Error::ThreadPanicked)??;

            Ok(())
        }
        Err(e) => {
            scu_stream.shutdown(Shutdown::Both)?;
            println!("error connection to destination SCP: {}", e);
            Err(e)?
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
                    println!("error: {}", e);
                }
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
