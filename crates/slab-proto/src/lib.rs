pub mod slab {
    pub mod ipc {
        pub mod v1 {
            tonic::include_proto!("slab.ipc.v1");
        }
    }
}

pub mod openai;

/// Slab-owned agent harness protocol (JSON-RPC 2.0 over WebSocket).
///
/// This is the canonical control-plane contract: a clean thread/turn/item model
/// with an explicit `Op → Started → Delta* → Completed | Error` lifecycle. It is
/// deliberately independent of the OpenAI-Responses DTOs in [`openai`]; the
/// conversion from slab-agent's internal events lives in `slab-app-core`.
pub mod harness;
