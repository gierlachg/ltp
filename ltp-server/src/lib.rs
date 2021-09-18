use std::error::Error;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::net::SocketAddr;

use log::info;
use thiserror::Error;

use crate::control::GroundControl;
use crate::protocol::{Protocol, StringProtocol};
use crate::reader::Reader;
use crate::tcp::{Connection, Connections, StringConnection};
use crate::Command::{GET, SHUTDOWN};

mod control;
mod protocol;
mod reader;
mod tcp;

pub async fn run(endpoint: SocketAddr, path: &str) -> Result<(), LTPError> {
    let control = GroundControl::init();
    let reader = Reader::new(path)?;
    let mut connections = Connections::bind(endpoint, control.clone(), StringConnection::new).await?;
    let protocol = StringProtocol::new();

    info!("LTP server started.");
    while let Some(connection) = connections.next_connection().await {
        let mut worker = Worker::new(control.clone(), connection, reader.clone(), protocol.clone());
        tokio::spawn(async move { worker.run().await });
    }
    info!("Shutting down LTP server.");

    Ok(())
}

struct Worker<T, C: Connection<T>, P: Protocol<T>> {
    control: GroundControl,
    connection: C,
    reader: Reader,
    protocol: P,
    _marker: PhantomData<T>,
}

impl<T, C: Connection<T>, P: Protocol<T>> Worker<T, C, P> {
    fn new(control: GroundControl, connection: C, reader: Reader, protocol: P) -> Self {
        Worker {
            control,
            connection,
            reader,
            protocol,
            _marker: PhantomData,
        }
    }

    async fn run(&mut self) {
        while let Some(command) = self.connection.read().await {
            match self.protocol.decode(command) {
                SHUTDOWN => self.control.signal_shutdown(),
                GET(line_number) => {
                    let line = self.reader.read(line_number).await;
                    let payload = self.protocol.encode(line);
                    if let Err(_) = self.connection.write(payload).await {
                        break;
                    }
                }
                _ => break,
            }
        }
    }
}

enum Command {
    QUIT,
    SHUTDOWN,
    GET(u64),
    ERRONEOUS,
    UNKNOWN,
}

#[derive(Error, Debug)]
pub enum LTPError {
    #[error("error")]
    GenericFailure(#[from] Box<dyn Error + Send + Sync>),
    #[error("io error")]
    GenericIOFailure(#[from] std::io::Error),
}
