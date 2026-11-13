use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::util::compact;

/// 对应 shared/types.ts 的 NewsItem。id / pubDate 允许 string|number，故用 Value。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: Value,
    pub title: String,
    pub url: String,
    #[serde(rename = "mobileUrl", skip_serializing_if = "Option::is_none")]
    pub mobile_url: Option<String>,
    #[serde(rename = "pubDate", skip_serializing_if = "Option::is_none")]
    pub pub_date: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

impl NewsItem {
    pub fn new(id: impl Into<Value>, title: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            url: url.into(),
            mobile_url: None,
            pub_date: None,
            extra: None,
        }
    }

    pub fn pubv(mut self, v: impl Into<Value>) -> Self {
        let v = v.into();
        self.pub_date = if v.is_null() { None } else { Some(v) };
        self
    }

    pub fn mobile(mut self, v: impl Into<String>) -> Self {
        self.mobile_url = Some(v.into());
        self
    }

    /// extra 经过 compact：JS 里 undefined 字段不会出现，空对象也不输出。
    pub fn extra(mut self, v: Value) -> Self {
        let v = compact(v);
        let empty = v.as_object().map(|m| m.is_empty()).unwrap_or(true);
        self.extra = if empty { None } else { Some(v) };
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceResponse {
    pub status: &'static str,
    pub id: String,
    #[serde(rename = "updatedTime")]
    pub updated_time: i64,
    pub items: Vec<NewsItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<Value>,
}

/// /api/s 原样携带的 info 字段。
pub fn info() -> Value {
    json!({
        "LICENCE": "MIT",
        "Github": "https://github.com/ourongxing/newsnow",
        "Sponsorship": "If you rely on this service, sponsorship is welcome to help it run for the long term. Scan the QR code https://raw.githubusercontent.com/ourongxing/newsnow/main/screenshots/reward.gif"
    })
}
