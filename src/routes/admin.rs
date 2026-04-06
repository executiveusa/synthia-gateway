// Minimal admin dashboard — dark navy UI, no external dependencies
// Protected by gateway key. Shows providers, spend, model aliases.

use axum::{extract::State, routing::get, response::Html, Router};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(admin_ui))
}

async fn admin_ui(State(state): State<AppState>) -> Html<String> {
    let spend = *state.daily_spend.read().await;
    let budget = state.config.circuit_breaker.daily_budget_usd;
    let pct = ((spend / budget) * 100.0).round();
    let active = state.config.active_provider_count();

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>SYNTHIA™ Gateway</title>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ background: #08111f; color: #f2ece0; font-family: 'DM Sans', system-ui, sans-serif; padding: 32px; }}
  h1 {{ font-size: 1.5rem; color: #c9a84c; margin-bottom: 8px; letter-spacing: -0.02em; }}
  .sub {{ color: rgba(242,236,224,0.4); font-size: 13px; margin-bottom: 32px; }}
  .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; margin-bottom: 32px; }}
  .card {{ background: rgba(255,255,255,0.05); border: 1px solid rgba(201,168,76,0.2); border-radius: 12px; padding: 20px; }}
  .card-label {{ font-size: 10px; letter-spacing: 0.2em; text-transform: uppercase; color: rgba(242,236,224,0.4); margin-bottom: 8px; }}
  .card-value {{ font-size: 1.8rem; font-weight: 600; color: #c9a84c; }}
  .card-sub {{ font-size: 12px; color: rgba(242,236,224,0.4); margin-top: 4px; }}
  .bar {{ background: rgba(255,255,255,0.1); border-radius: 4px; height: 6px; margin-top: 12px; overflow: hidden; }}
  .bar-fill {{ height: 100%; border-radius: 4px; background: linear-gradient(90deg, #c9a84c, #e2bf6a); }}
  table {{ width: 100%; border-collapse: collapse; }}
  th {{ text-align: left; font-size: 10px; letter-spacing: 0.15em; text-transform: uppercase; color: rgba(242,236,224,0.4); padding: 8px 12px; }}
  td {{ padding: 10px 12px; border-top: 1px solid rgba(255,255,255,0.05); font-size: 13px; }}
  .dot {{ display: inline-block; width: 6px; height: 6px; border-radius: 50%; margin-right: 6px; }}
  .dot.on {{ background: #4ade80; }}
  .dot.off {{ background: rgba(255,255,255,0.2); }}
  code {{ background: rgba(255,255,255,0.08); padding: 2px 6px; border-radius: 4px; font-size: 12px; }}
</style>
</head>
<body>
<h1>◈ SYNTHIA™ Gateway</h1>
<div class="sub">Kupuri Media™ · v{version} · {active} providers active</div>

<div class="grid">
  <div class="card">
    <div class="card-label">Today's Spend</div>
    <div class="card-value">${spend:.4}</div>
    <div class="card-sub">of ${budget:.2} daily budget</div>
    <div class="bar"><div class="bar-fill" style="width:{pct}%"></div></div>
  </div>
  <div class="card">
    <div class="card-label">Budget Remaining</div>
    <div class="card-value">${remaining:.4}</div>
    <div class="card-sub">{pct}% used today</div>
  </div>
  <div class="card">
    <div class="card-label">Active Providers</div>
    <div class="card-value">{active}</div>
    <div class="card-sub">of 9 configured</div>
  </div>
</div>

<table>
<thead><tr>
  <th>Provider</th><th>Status</th><th>Default Model</th><th>Key Set</th>
</tr></thead>
<tbody>
{provider_rows}
</tbody>
</table>

<div style="margin-top:32px;color:rgba(242,236,224,0.25);font-size:11px;">
  Endpoints: <code>/v1/chat/completions</code> · <code>/anthropic/v1/messages</code> · <code>/synthia/spend</code> · <code>/v1/models</code>
</div>
</body></html>"#,
        version = env!("CARGO_PKG_VERSION"),
        active = active,
        spend = spend,
        budget = budget,
        pct = pct,
        remaining = budget - spend,
        provider_rows = build_provider_rows(&state),
    );

    Html(html)
}

fn build_provider_rows(state: &AppState) -> String {
    let providers: Vec<(&str, Option<(bool, String, bool)>)> = vec![
        (
            "anthropic",
            state.config.providers.anthropic.as_ref().map(|p| {
                (p.enabled, p.default_model.clone(), p.api_key.is_some())
            }),
        ),
        (
            "openai",
            state.config.providers.openai.as_ref().map(|p| {
                (p.enabled, p.default_model.clone(), p.api_key.is_some())
            }),
        ),
        (
            "gemini",
            state.config.providers.gemini.as_ref().map(|p| {
                (p.enabled, p.default_model.clone(), p.api_key.is_some())
            }),
        ),
        (
            "nvidia",
            state.config.providers.nvidia.as_ref().map(|p| {
                (p.enabled, p.default_model.clone(), p.api_key.is_some())
            }),
        ),
        (
            "inception",
            state.config.providers.inception.as_ref().map(|p| {
                (p.enabled, p.default_model.clone(), p.api_key.is_some())
            }),
        ),
        (
            "zai",
            state.config.providers.zai.as_ref().map(|p| {
                (p.enabled, p.default_model.clone(), p.api_key.is_some())
            }),
        ),
        (
            "ollama",
            state.config.providers.ollama.as_ref().map(|p| {
                (p.enabled, p.default_model.clone(), false)
            }),
        ),
        (
            "openrouter",
            state.config.providers.openrouter.as_ref().map(|p| {
                (p.enabled, p.default_model.clone(), p.api_key.is_some())
            }),
        ),
        (
            "byokey",
            state.config.providers.byokey.as_ref().map(|p| {
                (p.enabled, "subscription".into(), true)
            }),
        ),
    ];

    providers
        .iter()
        .map(|(name, info)| match info {
            Some((enabled, model, has_key)) => format!(
                "<tr><td><span class='dot {dot}'></span>{name}</td><td>{status}</td><td><code>{model}</code></td><td>{key}</td></tr>",
                dot = if *enabled { "on" } else { "off" },
                name = name,
                status = if *enabled { "enabled" } else { "disabled" },
                model = model,
                key = if *has_key { "✓" } else { "—" }
            ),
            None => format!(
                "<tr><td><span class='dot off'></span>{name}</td><td>not configured</td><td>—</td><td>—</td></tr>",
                name = name
            ),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
