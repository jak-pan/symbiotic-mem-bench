//! Dashboard backend for the membench benchmark explorer.
//!
//! A small axum service over the same `runs/` and `records/` files the CLI
//! reads. It exposes the normalized index, run detail, the merged question
//! browser, artifact paging, run comparison, the leaderboard, and a
//! command-preview runner. Static SPA assets are served from `--dist`.
//!
//! Run ids are repo-relative run-folder paths (which contain slashes), so
//! run-scoped endpoints take the id as an `?id=` query parameter rather than a
//! path segment.

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use clap::Parser;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use symbiotic_mem_bench::{
    BenchQueueEvent, artifacts, compare, cost, leaderboard, live, registry, runner,
    summarize_queue_timing,
};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Parser)]
#[command(name = "membench-server")]
#[command(about = "Dashboard backend for the membench benchmark explorer")]
struct Cli {
    /// Port to bind.
    #[arg(long, default_value_t = 8787)]
    port: u16,
    /// Repository root; run ids are made relative to it.
    #[arg(long, default_value = env!("CARGO_MANIFEST_DIR"))]
    repo_root: PathBuf,
    /// Registry roots to scan. Defaults to `<repo>/runs` and `<repo>/records`.
    #[arg(long)]
    root: Vec<PathBuf>,
    /// Built SPA directory to serve.
    #[arg(long)]
    dist: Option<PathBuf>,
}

struct AppState {
    repo_root: PathBuf,
    roots: Vec<PathBuf>,
}

impl AppState {
    fn scan(&self) -> Vec<registry::RunRecord> {
        registry::scan_registry(&self.roots, &self.repo_root)
    }

    fn find(&self, run_id: &str) -> Option<registry::RunRecord> {
        self.scan()
            .into_iter()
            .find(|record| record.run_id == run_id)
    }

    fn pending(&self) -> Vec<registry::PendingRun> {
        registry::scan_pending(&self.roots, &self.repo_root)
    }
}

type Shared = Arc<AppState>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo_root = cli.repo_root.clone();
    let roots = if cli.root.is_empty() {
        vec![repo_root.join("runs"), repo_root.join("records")]
    } else {
        cli.root.clone()
    };
    let dist = cli.dist.unwrap_or_else(|| repo_root.join("dashboard/dist"));

    let state: Shared = Arc::new(AppState {
        repo_root: repo_root.clone(),
        roots,
    });

    let api = Router::new()
        .route("/health", get(health))
        .route("/runs", get(list_runs))
        .route("/pending", get(pending_handler))
        .route("/leaderboard", get(leaderboard_handler))
        .route("/run", get(run_detail))
        .route("/run/live", get(run_live))
        .route("/run/questions", get(run_questions))
        .route("/run/artifact", get(run_artifact))
        .route("/run/traces", get(run_traces))
        .route("/compare", get(compare_handler))
        .route("/runner/schema", get(runner_schema))
        .route("/runner/plan", post(runner_plan))
        .with_state(state.clone());

    let mut app = Router::new()
        .nest("/api", api)
        .layer(CorsLayer::permissive());

    // Serve the built SPA when present; index.html backs unknown asset paths.
    if dist.is_dir() {
        let index = dist.join("index.html");
        app = app.fallback_service(ServeDir::new(&dist).fallback(ServeFile::new(index)));
        eprintln!("serving SPA from {}", dist.display());
    } else {
        eprintln!(
            "note: SPA dir {} not found; serving API only (run `npm run build` in dashboard/)",
            dist.display()
        );
    }

    let addr = SocketAddr::from(([127, 0, 0, 1], cli.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("membench-server listening on http://{addr}");
    eprintln!("registry roots: {:?}", state.roots);
    axum::serve(listener, app).await?;
    Ok(())
}

fn err(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": message.into() })))
}

async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true, "service": "membench-server" }))
}

#[derive(Deserialize)]
struct RunsQuery {
    system: Option<String>,
    benchmark: Option<String>,
    origin: Option<String>,
}

async fn list_runs(
    State(state): State<Shared>,
    Query(query): Query<RunsQuery>,
) -> impl IntoResponse {
    let summaries: Vec<registry::RunSummary> = state
        .scan()
        .iter()
        .map(registry::summarize)
        .filter(|summary| {
            query
                .system
                .as_ref()
                .is_none_or(|system| &summary.system == system)
                && query
                    .benchmark
                    .as_ref()
                    .is_none_or(|benchmark| &summary.benchmark == benchmark)
                && query
                    .origin
                    .as_ref()
                    .is_none_or(|origin| &summary.origin == origin)
        })
        .collect();
    Json(json!({ "runs": summaries }))
}

async fn pending_handler(State(state): State<Shared>) -> impl IntoResponse {
    Json(json!({ "pending": state.pending() }))
}

async fn run_live(
    State(state): State<Shared>,
    Query(query): Query<IdQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let pending = state
        .pending()
        .into_iter()
        .find(|run| run.run_id == query.id)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "pending run not found"))?;
    let run_root = state.repo_root.join(&pending.run_id);
    let detail = live::live_detail(&run_root);
    Ok(Json(json!({ "pending": pending, "detail": detail })))
}

#[derive(Deserialize)]
struct LeaderboardQuery {
    benchmark: Option<String>,
    limit: Option<u64>,
}

async fn leaderboard_handler(
    State(state): State<Shared>,
    Query(query): Query<LeaderboardQuery>,
) -> impl IntoResponse {
    let summaries: Vec<registry::RunSummary> =
        state.scan().iter().map(registry::summarize).collect();
    let mut cohorts = leaderboard::build_cohorts(summaries);
    if let Some(benchmark) = &query.benchmark {
        cohorts.retain(|cohort| &cohort.benchmark == benchmark);
    }
    if let Some(limit) = query.limit {
        cohorts.retain(|cohort| cohort.limit == Some(limit));
    }
    Json(json!({ "cohorts": cohorts }))
}

#[derive(Deserialize)]
struct IdQuery {
    id: String,
}

async fn run_detail(
    State(state): State<Shared>,
    Query(query): Query<IdQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let record = state
        .find(&query.id)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "run not found"))?;
    let summary = registry::summarize(&record);
    let cohort_fields = registry::compute_cohort_fields(&record.run_root, &record.params);
    let cost_rollup = cost::rollup_model_traces(&record.run_root);
    Ok(Json(json!({
        "summary": summary,
        "report": record.report,
        "params": record.params,
        "cohort": cohort_fields,
        "cost": cost_rollup,
    })))
}

async fn run_questions(
    State(state): State<Shared>,
    Query(query): Query<IdQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let record = state
        .find(&query.id)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "run not found"))?;
    let rows = artifacts::question_rows(&record.run_root);
    Ok(Json(json!({ "total": rows.len(), "questions": rows })))
}

#[derive(Deserialize)]
struct ArtifactQuery {
    id: String,
    kind: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

/// Map an artifact kind to its file name and whether it is line-delimited.
fn artifact_file(kind: &str) -> Option<(&'static str, bool)> {
    Some(match kind {
        "hypotheses" => ("hypotheses.jsonl", true),
        "verdicts" => ("verdicts.jsonl", true),
        "partial_verdicts" => ("partial-verdicts.jsonl", true),
        "provenance" => ("provenance.jsonl", true),
        "memory_traces" => ("memory-traces.jsonl", true),
        "model_traces" => ("model-traces.jsonl", true),
        "scored" => ("scored.json", false),
        "score_summary" => ("score-summary.json", false),
        _ => return None,
    })
}

async fn run_artifact(
    State(state): State<Shared>,
    Query(query): Query<ArtifactQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let record = state
        .find(&query.id)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "run not found"))?;
    let (file, is_jsonl) = artifact_file(&query.kind)
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "unknown artifact kind"))?;
    let path = record.run_root.join("artifacts").join(file);

    if !is_jsonl {
        let raw = std::fs::read_to_string(&path)
            .map_err(|_| err(StatusCode::NOT_FOUND, "artifact not present"))?;
        let value: Value = serde_json::from_str(&raw)
            .map_err(|error| err(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        return Ok(Json(json!({ "kind": query.kind, "json": value })));
    }

    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(200).min(2000);
    let (total, rows) = read_jsonl_values(&path, offset, limit);
    Ok(Json(json!({
        "kind": query.kind,
        "total": total,
        "offset": offset,
        "limit": limit,
        "rows": rows,
    })))
}

fn read_jsonl_values(path: &Path, offset: usize, limit: usize) -> (usize, Vec<Value>) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return (0, Vec::new());
    };
    let lines: Vec<&str> = raw.lines().filter(|line| !line.trim().is_empty()).collect();
    let total = lines.len();
    let rows = lines
        .into_iter()
        .skip(offset)
        .take(limit)
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect();
    (total, rows)
}

/// Cap on memory-trace rows returned at once.
const TRACE_ROW_CAP: usize = 4000;

async fn run_traces(
    State(state): State<Shared>,
    Query(query): Query<IdQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let record = state
        .find(&query.id)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "run not found"))?;
    let root = &record.run_root;

    let (memory_total, memory_rows) = read_jsonl_values(
        &root.join("artifacts").join("memory-traces.jsonl"),
        0,
        TRACE_ROW_CAP,
    );
    let model_rollup = cost::rollup_model_traces(root);

    // Provider/model queue timing, when provider-backed runs emit queue JSONL.
    let queue_timing = read_queue_timing(root);
    let workflow_queue = read_workflow_queue(root);

    Ok(Json(json!({
        "memory_traces": {
            "total": memory_total,
            "truncated": memory_total > memory_rows.len(),
            "rows": memory_rows,
        },
        "model_rollup": model_rollup,
        "queue_timing": queue_timing,
        "workflow_queue": workflow_queue,
    })))
}

fn read_queue_timing(run_root: &Path) -> Option<Value> {
    let path = run_root
        .join("provider-queue")
        .join("model-queue-traces.jsonl");
    let raw = std::fs::read_to_string(&path).ok()?;
    let mut events = Vec::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<BenchQueueEvent>(line) {
            events.push(event);
        }
    }
    if events.is_empty() {
        return None;
    }
    serde_json::to_value(summarize_queue_timing(&events)).ok()
}

fn read_workflow_queue(run_root: &Path) -> Option<Value> {
    let workflow_root = run_root.join("workflow");
    if !workflow_root.is_dir() {
        return None;
    }

    let mut queue_paths = Vec::new();
    let direct = workflow_root.join("queue.sqlite");
    if direct.is_file() {
        queue_paths.push(direct);
    }
    if let Ok(entries) = std::fs::read_dir(&workflow_root) {
        for entry in entries.flatten() {
            let path = entry.path().join("queue.sqlite");
            if path.is_file() {
                queue_paths.push(path);
            }
        }
    }

    let databases: Vec<Value> = queue_paths
        .iter()
        .filter_map(|path| read_workflow_queue_db(run_root, path))
        .collect();
    if databases.is_empty() {
        None
    } else {
        Some(json!({ "databases": databases }))
    }
}

fn read_workflow_queue_db(run_root: &Path, path: &Path) -> Option<Value> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let items_by_status = query_count_map(
        &conn,
        "select status, count(*) from queue_items group by status",
    );
    let events_by_status = query_count_map(
        &conn,
        "select status, count(*) from queue_events group by status",
    );
    let queues = query_count_map(
        &conn,
        "select queue_id, count(*) from queue_items group by queue_id",
    );
    let total_items = items_by_status.values().sum::<u64>();
    let total_events = scalar_count(&conn, "select count(*) from queue_events").unwrap_or(0);
    let retried_items =
        scalar_count(&conn, "select count(*) from queue_items where attempt > 1").unwrap_or(0);
    let max_attempt =
        scalar_count(&conn, "select coalesce(max(attempt), 0) from queue_items").unwrap_or(0);
    let recent_errors = recent_workflow_errors(&conn);
    let recent_events = recent_workflow_events(&conn);
    Some(json!({
        "path": path.strip_prefix(run_root).unwrap_or(path).display().to_string(),
        "total_items": total_items,
        "total_events": total_events,
        "items_by_status": items_by_status,
        "events_by_status": events_by_status,
        "queues": queues,
        "retried_items": retried_items,
        "max_attempt": max_attempt,
        "recent_errors": recent_errors,
        "recent_events": recent_events,
    }))
}

fn query_count_map(conn: &rusqlite::Connection, sql: &str) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let Ok(mut stmt) = conn.prepare(sql) else {
        return out;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    }) else {
        return out;
    };
    for row in rows.flatten() {
        out.insert(row.0, row.1.max(0) as u64);
    }
    out
}

fn scalar_count(conn: &rusqlite::Connection, sql: &str) -> Option<u64> {
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .ok()
        .map(|count| count.max(0) as u64)
}

fn recent_workflow_errors(conn: &rusqlite::Connection) -> Vec<Value> {
    let Ok(mut stmt) = conn.prepare(
        "select item_id, queue_id, kind, status, attempt, last_error \
         from queue_items \
         where last_error is not null and last_error != '' \
         order by updated_at desc \
         limit 20",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok(json!({
            "item_id": row.get::<_, String>(0)?,
            "queue_id": row.get::<_, String>(1)?,
            "kind": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "attempt": row.get::<_, i64>(4)?.max(0) as u64,
            "error": row.get::<_, String>(5)?,
        }))
    }) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

fn recent_workflow_events(conn: &rusqlite::Connection) -> Vec<Value> {
    let Ok(mut stmt) = conn.prepare(
        "select item_id, queue_id, kind, status, attempt, timestamp, error \
         from queue_events \
         order by event_id desc \
         limit 40",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok(json!({
            "item_id": row.get::<_, String>(0)?,
            "queue_id": row.get::<_, String>(1)?,
            "kind": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "attempt": row.get::<_, i64>(4)?.max(0) as u64,
            "timestamp": row.get::<_, String>(5)?,
            "error": row.get::<_, Option<String>>(6)?,
        }))
    }) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

#[derive(Deserialize)]
struct CompareQuery {
    base: String,
    cand: String,
}

async fn compare_handler(
    State(state): State<Shared>,
    Query(query): Query<CompareQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let base = state
        .find(&query.base)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "base run not found"))?;
    let cand = state
        .find(&query.cand)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "candidate run not found"))?;
    let result = compare::compare_runs(&base.run_root, &cand.run_root);
    Ok(Json(json!({
        "base": registry::summarize(&base),
        "candidate": registry::summarize(&cand),
        "result": result,
    })))
}

async fn runner_schema(State(state): State<Shared>) -> impl IntoResponse {
    // Enrich enum options with values observed in existing runs so the form
    // reflects the configurations actually used here.
    let records = state.scan();
    let mut observed: HashMap<String, Vec<String>> = HashMap::new();
    for record in &records {
        for key in [
            "distiller",
            "embedder",
            "store",
            "query_planner",
            "scorer",
            "dataset",
            "oracle",
            "memory_config",
            "symem_bin",
        ] {
            if let Some(value) = record.params.get(key).and_then(Value::as_str)
                && !value.is_empty()
            {
                let entry = observed.entry(key.to_string()).or_default();
                if !entry.contains(&value.to_string()) {
                    entry.push(value.to_string());
                }
            }
        }
    }

    let mut fields: Vec<Value> = runner::symem_param_schema()
        .into_iter()
        .map(|field| {
            let mut value = serde_json::to_value(&field).unwrap_or(Value::Null);
            if let Some(seen) = observed.get(&field.name) {
                value["observed"] = json!(seen);
            }
            value
        })
        .collect();
    fields.sort_by(|a, b| {
        a["group"]
            .as_str()
            .unwrap_or("")
            .cmp(b["group"].as_str().unwrap_or(""))
    });

    Json(json!({
        "system": "symbiotic-memory",
        "benchmark": "long-mem-eval",
        "fields": fields,
    }))
}

async fn runner_plan(State(state): State<Shared>, Json(params): Json<Value>) -> impl IntoResponse {
    let plan = runner::plan_from_params(&params, &state.repo_root);
    Json(serde_json::to_value(plan.preview()).unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_queue_summary_reads_sqlite_state() {
        let temp = tempfile::tempdir().unwrap();
        let queue_dir = temp.path().join("workflow").join("longmemeval");
        std::fs::create_dir_all(&queue_dir).unwrap();
        let db_path = queue_dir.join("queue.sqlite");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r#"
            create table queue_items (
                item_id text primary key,
                queue_id text not null,
                kind text not null,
                payload_json text not null,
                status text not null,
                attempt integer not null,
                max_attempts integer not null,
                run_after text not null,
                lease_owner text,
                lease_until text,
                idempotency_key text,
                last_error text,
                created_at text not null,
                updated_at text not null
            );
            create table queue_events (
                event_id integer primary key autoincrement,
                item_id text not null,
                queue_id text not null,
                kind text not null,
                status text not null,
                attempt integer not null,
                timestamp text not null,
                error text
            );
            insert into queue_items values (
                'item-1', 'workflow:longmemeval', 'longmemeval.row', '{}',
                'succeeded', 2, 4, '2026-01-01T00:00:00Z', null, null,
                null, '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:02Z'
            );
            insert into queue_events
                (item_id, queue_id, kind, status, attempt, timestamp, error)
            values
                ('item-1', 'workflow:longmemeval', 'longmemeval.row', 'pending', 0, '2026-01-01T00:00:00Z', null),
                ('item-1', 'workflow:longmemeval', 'longmemeval.row', 'running', 1, '2026-01-01T00:00:01Z', null),
                ('item-1', 'workflow:longmemeval', 'longmemeval.row', 'succeeded', 2, '2026-01-01T00:00:02Z', null);
            "#,
        )
        .unwrap();

        let summary = read_workflow_queue(temp.path()).unwrap();
        let db = &summary["databases"][0];
        assert_eq!(db["total_items"], json!(1));
        assert_eq!(db["total_events"], json!(3));
        assert_eq!(db["items_by_status"]["succeeded"], json!(1));
        assert_eq!(db["retried_items"], json!(1));
        assert_eq!(db["max_attempt"], json!(2));
    }
}
