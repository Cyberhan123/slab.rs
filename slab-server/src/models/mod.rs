//! Request / response DTO (Data Transfer Object) types.
//!
//! These types are used by Axum handlers for JSON (de)serialisation and are
//! annotated with [`utoipa`] attributes to generate an OpenAPI 3.0 schema.

pub mod management;
pub mod openai;
