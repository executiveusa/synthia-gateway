-- SYNTHIA Gateway — Database Schema
-- Tracks spend, provider health, and request logs

CREATE TABLE IF NOT EXISTS spend_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    provider    TEXT NOT NULL,
    model       TEXT NOT NULL,
    agent_id    TEXT,              -- which agent called (hermes, maxx, claude-code)
    input_tokens  INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    cost_usd    REAL DEFAULT 0.0,
    created_at  TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_spend_provider_date
    ON spend_log(provider, created_at);

CREATE INDEX IF NOT EXISTS idx_spend_agent_date
    ON spend_log(agent_id, created_at);

CREATE TABLE IF NOT EXISTS provider_status (
    provider        TEXT PRIMARY KEY,
    enabled         INTEGER DEFAULT 1,
    failure_count   INTEGER DEFAULT 0,
    circuit_open    INTEGER DEFAULT 0,  -- 0=closed(healthy), 1=open(failing)
    last_failure_at TEXT,
    last_success_at TEXT,
    updated_at      TEXT DEFAULT (datetime('now'))
);

-- Pre-populate all known providers
INSERT OR IGNORE INTO provider_status (provider) VALUES
    ('anthropic'),
    ('openai'),
    ('gemini'),
    ('nvidia'),
    ('inception'),
    ('zai'),
    ('ollama'),
    ('openrouter'),
    ('byokey');

CREATE TABLE IF NOT EXISTS daily_budget (
    date        TEXT PRIMARY KEY,        -- YYYY-MM-DD
    spend_usd   REAL DEFAULT 0.0,
    budget_usd  REAL NOT NULL,
    halted      INTEGER DEFAULT 0,       -- 1 = circuit open, all requests blocked
    updated_at  TEXT DEFAULT (datetime('now'))
);
