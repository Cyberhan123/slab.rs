//! SQLite implementation of [`RequestStore`].
//!
//! Uses [`sqlx`] with the `sqlite` feature.  Migrations are run automatically
//! on startup via [`SqliteStore::connect`].
//!
//! # Migrations path
//!
//! `sqlx::migrate!("./migrations")` resolves the path **at compile time**
//! relative to `CARGO_MANIFEST_DIR` (the crate root), so the directory is
//! embedded into the binary.  The database file location is determined at
//! runtime by the `SLAB_DATABASE_URL` environment variable and is **not**
//! related to the current working directory at runtime.
//!
//! # Queries
//!
//! The `sqlx::query` (runtime-verified) form is used deliberately so that no
//! `DATABASE_URL` environment variable is needed at compile time.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{RequestRecord, RequestStore};

/// SQLite-backed request audit store.
#[derive(Clone, Debug)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// Open (or create) the SQLite database at `url` and run pending migrations.
    ///
    /// `url` should be a sqlx-compatible SQLite URL, e.g. `"sqlite://slab.db"`
    /// or `"sqlite://:memory:"` for tests.
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(url).await?;
        // Path is resolved relative to CARGO_MANIFEST_DIR at compile time.
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }
}

impl RequestStore for SqliteStore {
    async fn insert(&self, record: RequestRecord) -> Result<(), sqlx::Error> {
        let id = record.id.to_string();
        let created_at = record.created_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO request_log (id, method, path, status, latency_ms, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(&record.method)
        .bind(&record.path)
        .bind(record.status)
        .bind(record.latency_ms)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_response(
        &self,
        id: Uuid,
        status: i64,
        latency_ms: i64,
    ) -> Result<(), sqlx::Error> {
        let id_str = id.to_string();
        sqlx::query(
            "UPDATE request_log SET status = ?1, latency_ms = ?2 WHERE id = ?3",
        )
        .bind(status)
        .bind(latency_ms)
        .bind(&id_str)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get(&self, id: Uuid) -> Result<Option<RequestRecord>, sqlx::Error> {
        let id_str = id.to_string();
        let row: Option<(String, String, String, Option<i64>, Option<i64>, String)> =
            sqlx::query_as(
                "SELECT id, method, path, status, latency_ms, created_at \
                 FROM request_log WHERE id = ?1",
            )
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|(row_id, method, path, status, latency_ms, created_at)| {
            let parsed_id = row_id.parse::<Uuid>().unwrap_or(id);
            let parsed_dt = created_at
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now());
            RequestRecord {
                id: parsed_id,
                method,
                path,
                status,
                latency_ms,
                created_at: parsed_dt,
            }
        }))
    }
}
