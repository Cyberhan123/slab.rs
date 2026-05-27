#![allow(clippy::too_many_arguments)]
pub(crate) mod models;

///! Re-export generated DTOs directly at crate::openai::* to avoid deep paths.
///! these dtos write from crates\slab-proto\openai\openapi\openapi.yaml by hand.
///! these are not meant is all right if need, please see openapi.yaml and change the dto to the right way, and add the test case in tests folder
///! No client code or server code should be added here, only the dto and test case for it, if you want to add client or server code, please add it in the client or server module respectively
pub use models::*;

#[cfg(test)]
mod tests;
