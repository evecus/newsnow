use anyhow::Result;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::sync::{Arc, Mutex};
use tracing::warn;

use crate::model::NewsItem;

#[derive(Clone, Debug)]
pub struct CacheInfo {
    pub id: String,
    pub items: Vec<NewsItem>,
    pub updated: i64,
}

/// 表结构与原 Node 版完全一致：cache(id TEXT PRIMARY KEY, updated INTEGER, data TEXT)。
#[derive(Clone)]
pub struct Cache {
    inner: Option<Arc<Mutex<Connection>>>,
}

impl Cache {
    pub fn open(path: &str, enabled: bool) -> Result<Self> {
        if !enabled {
            return Ok(Self { inner: None });
        }
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;
             CREATE TABLE IF NOT EXISTS cache (
               id TEXT PRIMARY KEY,
               updated INTEGER,
               data TEXT
             );",
        )?;
        Ok(Self {
            inner: Some(Arc::new(Mutex::new(conn))),
        })
    }

    pub fn get(&self, id: &str) -> Option<CacheInfo> {
        let guard = self.inner.as_ref()?.lock().ok()?;
        let mut stmt = guard
            .prepare("SELECT id, data, updated FROM cache WHERE id = ?1")
            .ok()?;
        let row: Option<(String, String, i64)> = stmt
            .query_row(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()
            .ok()
            .flatten();
        let (id, data, updated) = row?;
        match serde_json::from_str::<Vec<NewsItem>>(&data) {
            Ok(items) => Some(CacheInfo { id, items, updated }),
            Err(e) => {
                warn!("cache decode failed for {id}: {e}");
                None
            }
        }
    }

    pub fn get_entire(&self, ids: &[String]) -> Vec<CacheInfo> {
        if ids.is_empty() {
            return Vec::new();
        }
        let Some(guard) = self.inner.as_ref().and_then(|x| x.lock().ok()) else {
            return Vec::new();
        };
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!("SELECT id, data, updated FROM cache WHERE id IN ({placeholders})");
        let mut stmt = match guard.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = match stmt.query_map(params_from_iter(ids.iter()), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        }) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        rows.filter_map(|row| {
            let (id, data, updated) = row.ok()?;
            let items = serde_json::from_str::<Vec<NewsItem>>(&data).ok()?;
            Some(CacheInfo { id, items, updated })
        })
        .collect()
    }

    pub fn set(&self, id: &str, items: &[NewsItem], updated: i64) {
        let data = serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string());
        let Some(conn) = self.inner.as_ref() else { return };
        match conn.lock() {
            Ok(guard) => {
                if let Err(e) = guard.execute(
                    "INSERT OR REPLACE INTO cache (id, data, updated) VALUES (?1, ?2, ?3)",
                    params![id, data, updated],
                ) {
                    warn!("cache set failed for {id}: {e}");
                }
            }
            Err(e) => warn!("cache lock poisoned: {e}"),
        }
    }
}
