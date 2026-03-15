/// Ports (interface) layer: inversion-of-control traits for engine backends.
///
/// These traits decouple the scheduler from concrete GGML FFI implementations,
/// allowing the scheduler to drive any conforming engine without knowing its
/// internal details.
pub mod engine;
