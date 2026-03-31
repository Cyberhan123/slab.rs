pub mod convert;

pub mod slab {
    pub mod ipc {
        pub mod v1 {
            tonic::include_proto!("slab.ipc.v1");
        }
    }
}
