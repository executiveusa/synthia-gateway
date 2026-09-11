use crate::circuit::CircuitBreaker;
use crate::config::GatewayConfig;
use crate::providers::router::Router;
use anyhow::Result;
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, SqlitePool};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<GatewayConfig>,
    pub router: Arc<Router>,
    pub db: SqlitePool,
    pub daily_spend: Arc<RwLock<f64>>,
    pub circuit: Arc<RwLock<CircuitBreaker>>,
}

impl AppState {
    pub async fn new(config: GatewayConfig) -> Result<Self> {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:///data/synthia-gateway.db".into());
        Self::new_with_db_url(config, &db_url).await
    }

    /// Same as [`AppState::new`] with an explicit database URL — used by the
    /// test suite with in-memory SQLite. In-memory pools are pinned to one
    /// connection so every pooled connection shares the same database.
    pub async fn new_with_db_url(config: GatewayConfig, db_url: &str) -> Result<Self> {

        // Ensure parent directory exists for absolute paths (sqlite:///abs/path)
        if let Some(path) = db_url.strip_prefix("sqlite://") {
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
        }

        let connect_opts = SqliteConnectOptions::from_str(db_url)?
            .create_if_missing(true);

        let max_connections = if db_url.contains(":memory:") { 1 } else { 5 };
        let db = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect_with(connect_opts)
            .await?;

        sqlx::migrate!("./migrations").run(&db).await?;

        // Load today's spend
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let spend: f64 = sqlx::query_scalar(
            "SELECT COALESCE(spend_usd, 0.0) FROM daily_budget WHERE date = ?"
        )
        .bind(&today)
        .fetch_optional(&db)
        .await?
        .unwrap_or(0.0);

        let router = Router::new(config.clone());

        let mut circuit = CircuitBreaker::new(
            config.circuit_breaker.failure_threshold,
            config.circuit_breaker.reset_seconds,
        );
        // Rehydrate last known provider health.
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT provider, circuit_open, failure_count FROM provider_status",
        )
        .fetch_all(&db)
        .await
        .unwrap_or_default();
        for (provider, open, failures) in rows {
            circuit.seed(&provider, open != 0, failures as u32);
        }

        Ok(Self {
            config: Arc::new(config),
            router: Arc::new(router),
            db,
            daily_spend: Arc::new(RwLock::new(spend)),
            circuit: Arc::new(RwLock::new(circuit)),
        })
    }

    /// Should traffic be sent to this provider right now?
    pub async fn circuit_allows(&self, provider: &str) -> bool {
        self.circuit.read().await.allows(provider)
    }

    pub async fn circuit_success(&self, provider: &str) {
        self.circuit.write().await.record_success(provider);
        self.persist_health(provider, true).await;
    }

    pub async fn circuit_failure(&self, provider: &str) {
        self.circuit.write().await.record_failure(provider);
        self.persist_health(provider, false).await;
    }

    /// Mirror in-memory circuit state into provider_status (best-effort,
    /// off the request path).
    async fn persist_health(&self, provider: &str, success: bool) {
        let (open, failures) = {
            let cb = self.circuit.read().await;
            let failures = cb
                .snapshot()
                .into_iter()
                .find(|s| s.provider == provider)
                .map(|s| s.consecutive_failures as i64)
                .unwrap_or(0);
            (cb.is_open(provider), failures)
        };
        let db = self.db.clone();
        let provider = provider.to_string();
        let ts_col = if success { "last_success_at" } else { "last_failure_at" };
        tokio::spawn(async move {
            let _ = sqlx::query(&format!(
                "INSERT INTO provider_status (provider, circuit_open, failure_count, {ts_col}, updated_at)
                 VALUES (?, ?, ?, datetime('now'), datetime('now'))
                 ON CONFLICT(provider) DO UPDATE SET
                   circuit_open = excluded.circuit_open,
                   failure_count = excluded.failure_count,
                   {ts_col} = excluded.{ts_col},
                   updated_at = datetime('now')"
            ))
            .bind(&provider)
            .bind(open as i64)
            .bind(failures)
            .execute(&db)
            .await;
        });
    }

    pub async fn is_budget_exceeded(&self) -> bool {
        let spend = *self.daily_spend.read().await;
        spend >= self.config.circuit_breaker.daily_budget_usd
    }

    pub async fn record_spend(
        &self,
        provider: &str,
        model: &str,
        agent_id: Option<&str>,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) {
        {
            let mut spend = self.daily_spend.write().await;
            *spend += cost_usd;
        }

        let db = self.db.clone();
        let provider = provider.to_string();
        let model = model.to_string();
        let agent_id = agent_id.map(String::from);
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let budget = self.config.circuit_breaker.daily_budget_usd;

        tokio::spawn(async move {
            let _ = sqlx::query(
                "INSERT INTO spend_log (provider, model, agent_id, input_tokens, output_tokens, cost_usd)
                 VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(&provider)
            .bind(&model)
            .bind(&agent_id)
            .bind(input_tokens)
            .bind(output_tokens)
            .bind(cost_usd)
            .execute(&db)
            .await;

            let _ = sqlx::query(
                "INSERT INTO daily_budget (date, spend_usd, budget_usd)
                 VALUES (?, ?, ?)
                 ON CONFLICT(date) DO UPDATE SET
                   spend_usd = spend_usd + excluded.spend_usd,
                   updated_at = datetime('now')"
            )
            .bind(&today)
            .bind(cost_usd)
            .bind(budget)
            .execute(&db)
            .await;
        });
    }
}
