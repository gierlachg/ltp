/// An utility helping with graceful shutdown of the system.
#[derive(Clone)]
pub(crate) struct GroundControl {
    signaler: tokio::sync::mpsc::UnboundedSender<()>,
    shutdown: tokio::sync::watch::Receiver<()>,
}

impl GroundControl {
    pub(crate) fn init() -> Self {
        let (signaler_tx, mut signaler_rx) = tokio::sync::mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
        tokio::spawn(async move {
            let _shutdown_tx = shutdown_tx;
            tokio::select! {
                _ = signaler_rx.recv() => {},
                _ = tokio::signal::ctrl_c() => {}
            }
        });
        GroundControl {
            signaler: signaler_tx,
            shutdown: shutdown_rx,
        }
    }

    pub(crate) fn signal_shutdown(&self) {
        // safety: fail send means ctrl-c already encountered
        self.signaler.send(()).unwrap_or(())
    }

    pub(crate) async fn await_shutdown(&mut self) -> () {
        // safety: failure means shutdown signal already received
        self.shutdown.changed().await.unwrap_or(())
    }
}
