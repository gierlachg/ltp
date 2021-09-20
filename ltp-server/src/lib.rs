use std::error::Error;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::net::SocketAddr;

use log::info;
use thiserror::Error;

use crate::control::GroundControl;
use crate::protocol::{Protocol, StringProtocol};
use crate::storage::Storage;
use crate::tcp::{Connection, Connections, StringConnections};
use crate::Request::{ERRONEOUS, GET, QUIT, SHUTDOWN};
use crate::Response::{ERROR, LINE};

mod control;
mod protocol;
mod storage;
mod tcp;

/// Serves individual lines from a static text file to clients over the network. The client-server protocol for this
/// system is the following:
///
/// * `GET <n>` => If <n> is a valid line number for the text file, `'OK\r\n'` is returned followed by the <n>th line from
/// the text file. If <n> is NOT a valid line number, `'ERR\r\n'` is returned. Note that the lines in the file are
/// indexed starting from 1, not 0.
/// * `QUIT` => This command disconnects the client.
/// * `SHUTDOWN` => This command shuts down the server.
///
/// The assumption is made that every line is newline (`'\n'`) terminated and that every character in the file is valid
/// ASCII.
pub async fn run(endpoint: SocketAddr, path: &str) -> Result<(), LTPError> {
    info!("Initializing LTP server (version: {}).", env!("CARGO_PKG_VERSION"));

    let storage = Storage::init(path)?;
    let control = GroundControl::init();
    let protocol = StringProtocol::new();
    let mut connections = StringConnections::from(endpoint, control.clone()).await?;

    info!("LTP server started.");

    while let Some(connection) = connections.next_connection().await {
        let mut worker = Worker::new(control.clone(), connection, storage.clone(), protocol.clone());
        tokio::spawn(async move { worker.run().await });
    }

    info!("Shutting down LTP server.");
    info!("LTP server shut down.");

    Ok(())
}

/// Request/response broker/handler/worker.
struct Worker<T, P: Protocol<T>, C: Connection<T>> {
    control: GroundControl,
    connection: C,
    storage: Storage,
    protocol: P,
    _marker: PhantomData<T>,
}

impl<T, P: Protocol<T>, C: Connection<T>> Worker<T, P, C> {
    fn new(control: GroundControl, connection: C, storage: Storage, protocol: P) -> Self {
        Worker {
            control,
            connection,
            storage,
            protocol,
            _marker: PhantomData,
        }
    }

    async fn run(&mut self) {
        while let Some(request) = self.connection.read().await {
            let response = match self.protocol.decode(request) {
                QUIT => break,
                SHUTDOWN => {
                    self.control.signal_shutdown();
                    break;
                }
                GET(line_number) => self.storage.read(line_number).await.map_or(ERROR, |line| LINE(line)),
                ERRONEOUS => ERROR,
            };
            let payload = self.protocol.encode(response);
            if let Err(_) = self.connection.write(payload).await {
                break;
            }
        }
    }
}

enum Request {
    QUIT,
    SHUTDOWN,
    GET(u64),
    ERRONEOUS,
}

enum Response {
    LINE(String),
    ERROR,
}

#[derive(Error, Debug)]
pub enum LTPError {
    #[error("error")]
    GenericFailure(#[from] Box<dyn Error + Send + Sync>),
    #[error("io error")]
    GenericIOFailure(#[from] std::io::Error),
}
