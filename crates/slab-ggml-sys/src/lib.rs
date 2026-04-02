#![allow(clippy::all)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

mod loader;

pub use loader::GGmlLoaderLib;

pub struct GGmlLib {
    pub base: GGmlBaseLib,
    /// Unfortunately, we have to import two dynamic libraries because the linking symbols are in different dynamic libraries.
    pub loader: GGmlLoaderLib,
}

impl GGmlLib {
    pub unsafe fn new<P1, P2>(base_path: P1, loader_path: P2) -> Result<Self, libloading::Error>
    where
        P1: AsRef<std::ffi::OsStr>,
        P2: AsRef<std::ffi::OsStr>,
    {
        Ok(Self {
            base: unsafe { GGmlBaseLib::new(base_path)? },
            loader: unsafe { GGmlLoaderLib::new(loader_path)? },
        })
    }

    pub unsafe fn from_library<L1, L2>(base: L1, loader: L2) -> Result<Self, ::libloading::Error>
    where
        L1: Into<::libloading::Library>,
        L2: Into<::libloading::Library>,
    {

        let base = GGmlBaseLib::from_library(base.into())?;
        let loader = GGmlLoaderLib::from_library(loader.into())?;
        Ok(Self { base, loader })
    }
}
