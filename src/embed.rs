use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// 前端 pnpm build 产物 frontend/dist 直接编译进单二进制（编译 Rust 前须先构建前端）。
/// 未开启 debug-embed 特性时，debug 构建直接从磁盘读取，改前端无需重编译 Rust。
#[derive(RustEmbed)]
#[folder = "frontend/dist/"]
pub struct Assets;

const NO_CACHE: &[&str] = &["index.html", "sw.js", "swx.js", "manifest.webmanifest"];

fn file_response(name: &str, data: &[u8]) -> Response {
    let mime = mime_guess::from_path(name).first_or_octet_stream();
    let cc = if NO_CACHE.contains(&name) {
        "no-cache, must-revalidate"
    } else {
        "public, max-age=31536000, immutable"
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, cc)
        .body(Body::from(data.to_vec()))
        .unwrap()
}

/// SPA fallback：未命中的路径返回 index.html；/api/* 未匹配则返回 404，不回落到页面。
pub async fn serve(uri: Uri) -> Response {
    let raw = uri.path().trim_start_matches('/');
    if raw == "api" || raw.starts_with("api/") {
        return (StatusCode::NOT_FOUND, "Not Found").into_response();
    }
    let target = if raw.is_empty() { "index.html" } else { raw };

    if let Some(file) = Assets::get(target) {
        return file_response(target, file.data.as_ref());
    }
    if let Some(file) = Assets::get("index.html") {
        return file_response("index.html", file.data.as_ref());
    }
    (StatusCode::NOT_FOUND, "Not Found").into_response()
}
