use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

/// sources.json 中我们只需要 redirect 和 interval；disable 等字段忽略。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SourceMeta {
    #[serde(default)]
    pub redirect: Option<String>,
    #[serde(default)]
    pub interval: Option<i64>,
}

pub struct Meta {
    pub sources: HashMap<String, SourceMeta>,
}

pub const DEFAULT_INTERVAL: i64 = 10 * 60 * 1000;

impl Meta {
    /// 元数据在编译期嵌入，运行时零配置文件。
    pub fn load() -> Result<Self> {
        let raw = include_str!("../data/sources.json");
        let sources: HashMap<String, SourceMeta> = serde_json::from_str(raw)?;
        Ok(Self { sources })
    }

    pub fn exists(&self, id: &str) -> bool {
        self.sources.contains_key(id)
    }

    /// 跟随 redirect（如 v2ex -> v2ex-share），返回最终可抓取的 id。
    pub fn resolve(&self, id: &str) -> Option<String> {
        let mut cur = id.to_string();
        for _ in 0..5 {
            let m = self.sources.get(&cur)?;
            match &m.redirect {
                Some(next) if next != &cur => cur = next.clone(),
                _ => return Some(cur),
            }
        }
        None
    }

    pub fn interval(&self, id: &str) -> i64 {
        self.sources
            .get(id)
            .and_then(|m| m.interval)
            .unwrap_or(DEFAULT_INTERVAL)
    }
}
