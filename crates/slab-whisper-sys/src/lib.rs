#![allow(clippy::all)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// SAFETY: WhisperLib wraps dynamically-loaded FFI function pointers.
// The underlying whisper.cpp library is designed for multi-threaded use,
// and exclusive access per context is enforced at a higher level.
unsafe impl Send for WhisperLib {}
unsafe impl Sync for WhisperLib {}
