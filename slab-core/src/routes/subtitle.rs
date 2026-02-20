
use axum::{
    routing::{ post},
    Router,
};

use crate::handlers::subtitle;

pub fn router() -> Router {
    Router::new()
        .route("/generate", post(subtitle::generate))
}