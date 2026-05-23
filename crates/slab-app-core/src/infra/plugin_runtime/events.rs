use tokio::sync::broadcast;

use slab_types::PluginEventPayload;

#[derive(Clone)]
pub struct PluginEventBus {
    tx: broadcast::Sender<PluginEventPayload>,
}

impl PluginEventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    pub fn publish(&self, payload: PluginEventPayload) {
        let _ = self.tx.send(payload);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PluginEventPayload> {
        self.tx.subscribe()
    }
}

impl Default for PluginEventBus {
    fn default() -> Self {
        Self::new()
    }
}
