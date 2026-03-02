//! Database abstraction layer.
//!
//! [`RequestStore`] defines the interface for persisting request audit records.
//! The default implementation is [`sqlite::SqliteStore`].  To swap to another
//! database (Postgres, MySQL, …), implement [`RequestStore`] for your new
//! type and change the concrete type in [`crate::state::AppState`].
//!
//! All trait methods use `impl Future` in their signatures (stable since Rust
//! 1.75) so no extra `async-trait` crate is required.

pub mod chat;
pub mod config;
pub mod dao;
pub mod model;
pub mod session;
pub mod task;

pub use dao::{ChatMessage, ChatSession, ModelCatalogRecord, TaskRecord};

pub use chat::ChatStore;
pub use config::ConfigStore;
pub use model::ModelStore;
pub use session::SessionStore;

pub use task::TaskStore;

use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct AnyStore {
    pool: sqlx::Pool<sqlx::Any>,
}

impl AnyStore {
    /// Open (or create) the SQLite database at `url` and run pending migrations.
    ///
    /// `url` should be a sqlx-compatible SQLite URL, e.g. `"sqlite://slab.db"`
    /// or `"sqlite://:memory:"` for tests.
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        sqlx::any::install_default_drivers();
        let options = sqlx::any::AnyConnectOptions::from_str(url)?;
        let pool = sqlx::AnyPool::connect_with(options).await?;
        // Path is resolved relative to CARGO_MANIFEST_DIR at compile time.
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }
}
