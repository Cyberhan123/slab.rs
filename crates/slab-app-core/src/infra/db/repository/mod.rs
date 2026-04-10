pub mod agent;
pub mod chat;
pub mod config;
pub mod model;
pub mod model_config_state;
pub mod model_download;
pub mod session;
pub mod task;
pub mod ui_state;

pub use chat::ChatStore;
pub use model::ModelStore;
pub use model_config_state::ModelConfigStateStore;
pub use model_download::ModelDownloadStore;
pub use session::SessionStore;
pub use task::TaskStore;
pub use ui_state::UiStateStore;

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
