pub mod slab {
    pub mod ipc {
        pub mod v1 {
            tonic::include_proto!("slab.ipc.v1");
        }
    }
}

pub mod openai;

// Re-export openai models at crate root for backward compatibility with generated code
pub use openai::models;
