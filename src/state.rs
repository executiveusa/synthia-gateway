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
}

impl AppState {
    pub async fn new(config: GatewayConfig) -> Result<Self> {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:///data/synthia-gateway.db".into());

        // Ensure parent directory exists for absolute paths (sqlite:///abs/path)
        if let Some(path) = db_url.strip_prefix("sqlite://") {
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
        }

        let connect_opts = SqliteConnectOptions::from_str(&db_url)?
            .create_if_missing(true);

        let db = SqlitePoolOptions::new()
            .max_connections(5)
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

        Ok(Self {
            config: Arc::new(config),
            router: Arc::new(router),
            db,
            daily_spend: Arc::new(RwLock::new(spend)),
        })
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
