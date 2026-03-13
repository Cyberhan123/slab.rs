pub mod config;
pub mod contexts;
pub(crate) mod dao;

pub use config::ConfigStore;
pub use contexts::chat::application::ports::ChatRepository;
pub use contexts::chat::domain::{ChatMessage, ChatSession};
pub use contexts::model::application::ports::ModelRepository;
pub use contexts::model::domain::ModelCatalogRecord;
pub use contexts::task::application::ports::TaskRepository;
pub use contexts::task::domain::{TaskRecord, TaskStatus};

use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct SqlxStore {
    pub(crate) pool: sqlx::Pool<sqlx::Any>,
}

impl SqlxStore {
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        sqlx::any::install_default_drivers();
        let options = sqlx::any::AnyConnectOptions::from_str(url)?;
        let pool = sqlx::AnyPool::connect_with(options).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }
}

pub type AnyStore = SqlxStore;
pub use ChatRepository as ChatStore;
pub use ChatRepository as SessionStore;
pub use ModelRepository as ModelStore;
pub use TaskRepository as TaskStore;

// Ensure SQLx implementations are linked.
use contexts::chat::infrastructure::persistence::sqlx as _chat_sqlx_impl;
use contexts::model::infrastructure::persistence::sqlx as _model_sqlx_impl;
use contexts::task::infrastructure::persistence::sqlx as _task_sqlx_impl;
