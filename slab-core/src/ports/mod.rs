/// Ports (interface) layer: inversion-of-control traits for engine backends.
///
/// This module contains two kinds of ports:
///
/// 1. **Scheduler-level engine ports** ([`engine`]) – low-level `Engine` and
///    `Worker` traits that decouple the scheduler from concrete GGML FFI
///    implementations, allowing the scheduler to drive any conforming engine
///    without knowing its internal details.
///
/// 2. **Capability ports** ([`capabilities`]) – high-level async traits
///    (`TextGenerationBackend`, `AudioTranscriptionBackend`,
///    `ImageGenerationBackend`, `ImageEmbeddingBackend`) that decouple
///    application layers (e.g. `slab-server`) from the concrete inference
///    backend (GGML, Candle, ONNX).  Upper-layer code programs exclusively
///    against these traits and never references engine-specific types.
pub mod capabilities;
pub mod engine;
