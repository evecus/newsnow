use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::cache::Cache;
use crate::ctx::Ctx;
use crate::meta::Meta;
use crate::model::{info, SourceResponse};
use crate::util::now_ms;

pub const TTL: i64 = 30 * 60 * 1000;

pub struct AppState {
    pub ctx: Ctx,
    pub cache: Cache,
    pub meta: Meta,
}

#[derive(Debug, Deserialize)]
pub struct SQuery {
    pub id: Option<String>,
    pub latest: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EntireBody {
    pub sources: Option<Vec<String>>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/s", get(s_handler))
        .route("/api/s/entire", post(entire_handler))
        .route("/api/latest", get(latest_handler))
        // 登录已被砍掉：固定 506，前端 useLogin 会关闭登录，useSync 会静默忽略。
        .route("/api/enable-login", get(disabled))
        .route("/api/login", get(disabled))
        .route("/api/oauth/github", get(disabled))
        .route("/api/me", get(me_handler))
        .route("/api/me/sync", get(disabled).post(disabled))
        .fallback(crate::embed::serve)
        .with_state(state)
}

fn ok(resp: SourceResponse) -> Response {
    Json(resp).into_response()
}

fn err(code: u16, msg: &str) -> Response {
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(json!({ "statusCode": code, "message": msg })),
    )
        .into_response()
}

fn success(id: String, updated_time: i64, items: Vec<crate::model::NewsItem>) -> Response {
    ok(SourceResponse {
        status: "success",
        id,
        updated_time,
        items,
        info: Some(info()),
    })
}

fn cached(id: String, updated_time: i64, items: Vec<crate::model::NewsItem>) -> Response {
    ok(SourceResponse {
        status: "cache",
        id,
        updated_time,
        items,
        info: Some(info()),
    })
}

/// 完整复刻 server/api/s/index.ts 的缓存状态机。
async fn s_handler(State(st): State<Arc<AppState>>, Query(q): Query<SQuery>) -> Response {
    let id = match q.id.as_deref() {
        Some(raw) => match st.meta.resolve(raw) {
            Some(resolved) if crate::sources::lookup(&resolved).is_some() => resolved,
            _ => return err(500, "Invalid source id"),
        },
        None => return err(500, "Invalid source id"),
    };
    let latest = q.latest.as_deref().map(|v| v != "false").unwrap_or(false);
    let now = now_ms();
    let cache = st.cache.get(&id);

    // interval 内：直接复用，但伪装成 success + now 时间戳。
    if let Some(c) = &cache {
        if now - c.updated < st.meta.interval(&id) {
            return success(id, now, c.items.clone());
        }
        // TTL 内且没有 latest：返回 cache 状态 + 旧时间戳。
        // 砍登录后所有人都能用 latest 强刷（等价原逻辑的 disabledLogin 分支）。
        if now - c.updated < TTL && !latest {
            return cached(id, c.updated, c.items.clone());
        }
    }

    match crate::sources::fetch(st.ctx.clone(), &id, now).await {
        Ok(mut items) => {
            items.truncate(30);
            if !items.is_empty() {
                // 对应 waitUntil：后台写库，不阻塞响应。
                let cache = st.cache.clone();
                let id2 = id.clone();
                let data = items.clone();
                tokio::spawn(async move {
                    let _ = tokio::task::spawn_blocking(move || cache.set(&id2, &data, now)).await;
                });
            }
            success(id, now, items)
        }
        Err(e) => {
            if let Some(c) = cache {
                cached(id, c.updated, c.items)
            } else {
                tracing::error!("fetch {id} failed: {e:#}");
                err(500, &e.to_string())
            }
        }
    }
}

async fn entire_handler(State(st): State<Arc<AppState>>, Json(body): Json<EntireBody>) -> Response {
    let ids: Vec<String> = body
        .sources
        .unwrap_or_default()
        .into_iter()
        .filter(|k| st.meta.exists(k))
        .collect();
    let now = now_ms();
    let caches = st.cache.get_entire(&ids);
    let list: Vec<SourceResponse> = caches
        .into_iter()
        .map(|c| SourceResponse {
            status: "cache",
            updated_time: if now - c.updated < st.meta.interval(&c.id) {
                now
            } else {
                c.updated
            },
            id: c.id,
            items: c.items,
            info: None,
        })
        .collect();
    Json(list).into_response()
}

async fn latest_handler() -> Response {
    Json(json!({ "v": env!("CARGO_PKG_VERSION") })).into_response()
}

async fn me_handler() -> Response {
    Json(json!({ "hello": "world" })).into_response()
}

/// 506 非标准状态码：无反代时直接透传，前端依赖它判断「禁止登录」。
async fn disabled() -> Response {
    (
        StatusCode::from_u16(506).unwrap(),
        "Server not configured, disable login",
    )
        .into_response()
}
