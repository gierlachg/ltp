use std::marker::PhantomData;
use std::net::SocketAddr;

use futures::SinkExt;
use log::warn;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::{select, spawn};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use crate::{GroundControl, LTPError};

pub(super) struct Connections<T, C: Connection<T>, F: Fn(GroundControl, TcpStream) -> C> {
    control: GroundControl,
    connections: UnboundedReceiver<TcpStream>,
    connection_factory: F,
    _marker: PhantomData<T>,
}

impl<T, C: Connection<T>, F: Fn(GroundControl, TcpStream) -> C> Connections<T, C, F> {
    pub(super) async fn bind(
        endpoint: SocketAddr,
        control: GroundControl,
        connection_factory: F,
    ) -> Result<Self, LTPError> {
        let listener = TcpListener::bind(&endpoint).await?;
        let (connections_tx, connections_rx) = mpsc::unbounded_channel(); // TODO: consider bounding the channel
        Self::spawn_listener(listener, connections_tx, control.clone());
        Ok(Connections {
            control,
            connections: connections_rx,
            connection_factory,
            _marker: PhantomData,
        })
    }

    fn spawn_listener(listener: TcpListener, connections: UnboundedSender<TcpStream>, mut control: GroundControl) {
        spawn(async move {
            loop {
                select! {
                    result = listener.accept() => match result {
                        // safety: should not ever fail as receiver is/should be dropped after this
                        Ok((stream, _)) => connections.send(stream).unwrap_or(()),
                        Err(e) => warn!("Error accepting connection; error = {:?}", e),
                    },
                    _ = control.await_shutdown() => break
                }
            }
        });
    }

    pub(super) async fn next_connection(&mut self) -> Option<C> {
        self.connections
            .recv()
            .await
            .map(|stream| (self.connection_factory)(self.control.clone(), stream))
    }
}

#[async_trait::async_trait]
pub(super) trait Connection<T> {
    async fn read(&mut self) -> Option<T>;

    async fn write(&mut self, payload: T) -> Result<(), LTPError>;
}

pub(super) struct StringConnection {
    closed: bool,
    control: GroundControl,
    lines: Framed<TcpStream, LinesCodec>,
}

impl StringConnection {
    pub(super) fn new(control: GroundControl, stream: TcpStream) -> Self {
        StringConnection {
            closed: false,
            control,
            lines: Framed::new(stream, LinesCodec::new()), // TODO: consider setting max length
        }
    }
}

#[async_trait::async_trait]
impl Connection<String> for StringConnection {
    async fn read(&mut self) -> Option<String> {
        if self.closed {
            None
        } else {
            select! {
                result = self.lines.next() => result.and_then(|result| result.ok()),
                _ = self.control.await_shutdown() => {
                    self.closed = true;
                    None
                },
            }
        }
    }

    async fn write(&mut self, payload: String) -> Result<(), LTPError> {
        self.lines
            .send(payload)
            .await
            .map_err(|e| LTPError::GenericFailure(e.into()))
    }
}
