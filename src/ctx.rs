use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, SET_COOKIE};
use reqwest::{Client, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

/// 对应 ofetch 的 $fetch.create：统一 UA、10s 超时、失败重试 3 次。
#[derive(Clone)]
pub struct Ctx {
    pub http: Client,
    /// ProductHunt API token（命令行 -token），为空则回退 RSS
    pub ph_token: std::sync::Arc<str>,
}

fn apply_headers(mut rb: RequestBuilder, headers: &[(&str, &str)]) -> RequestBuilder {
    for (k, v) in headers {
        if let (Ok(name), Ok(val)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v)) {
            rb = rb.header(name, val);
        }
    }
    rb
}

impl Ctx {
    pub fn new(ph_token: String) -> Self {
        let mut default = HeaderMap::new();
        default.insert(reqwest::header::USER_AGENT, HeaderValue::from_static(UA));
        default.insert(
            reqwest::header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
        );
        let http = Client::builder()
            .default_headers(default)
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(6))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .expect("failed to build http client");
        Self { http, ph_token: ph_token.into() }
    }

    /// 每次重试都重新构造请求，避免 RequestBuilder 无法 Clone 的问题。
    pub async fn send<F>(&self, f: F) -> Result<Response>
    where
        F: Fn(&Client) -> RequestBuilder,
    {
        let mut last = String::new();
        for attempt in 0..4 {
            let req = f(&self.http);
            match req.send().await {
                Ok(resp) => {
                    if resp.status().is_server_error() && attempt < 3 {
                        last = format!("HTTP {}", resp.status());
                        continue;
                    }
                    return Ok(resp);
                }
                Err(e) => last = e.to_string(),
            }
        }
        Err(anyhow!("request failed after 4 attempts: {last}"))
    }

    pub async fn get_raw(&self, url: &str) -> Result<Response> {
        self.send(|c| c.get(url)).await
    }

    pub async fn get_text(&self, url: &str) -> Result<String> {
        self.get_text_h(url, &[]).await
    }

    pub async fn get_text_h(&self, url: &str, headers: &[(&str, &str)]) -> Result<String> {
        let resp = self
            .send(|c| apply_headers(c.get(url), headers))
            .await?;
        Ok(resp.text().await?)
    }

    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self.send(|c| c.get(url)).await?;
        Ok(resp.bytes().await?.to_vec())
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        self.get_json_h(url, &[]).await
    }

    pub async fn get_json_h<T: DeserializeOwned>(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<T> {
        let text = self.get_text_h(url, headers).await?;
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn post_json<T: DeserializeOwned>(
        &self,
        url: &str,
        body: &Value,
        headers: &[(&str, &str)],
    ) -> Result<T> {
        let resp = self
            .send(|c| apply_headers(c.post(url).json(body), headers))
            .await?;
        let text = resp.text().await?;
        Ok(serde_json::from_str(&text)?)
    }

    /// header 值是运行时生成的（Cookie / X-App-Token）时用这个方法。
    pub async fn get_text_dyn(&self, url: &str, headers: &[(String, String)]) -> Result<String> {
        let resp = self
            .send(|c| {
                let mut rb = c.get(url);
                for (k, v) in headers {
                    if let (Ok(name), Ok(val)) =
                        (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v))
                    {
                        rb = rb.header(name, val);
                    }
                }
                rb
            })
            .await?;
        Ok(resp.text().await?)
    }

    pub async fn get_json_dyn<T: DeserializeOwned>(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<T> {
        let text = self.get_text_dyn(url, headers).await?;
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn post_text_dyn(
        &self,
        url: &str,
        body: String,
        headers: &[(String, String)],
    ) -> Result<String> {
        let resp = self
            .send(|c| {
                let mut rb = c.post(url).body(body.clone());
                for (k, v) in headers {
                    if let (Ok(name), Ok(val)) =
                        (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v))
                    {
                        rb = rb.header(name, val);
                    }
                }
                rb
            })
            .await?;
        Ok(resp.text().await?)
    }

    /// 对应 $fetch.raw(...).headers.getSetCookie()：拿到 set-cookie 里的键值对。
    pub async fn cookie_jar(&self, url: &str) -> Result<String> {
        let resp = self.get_raw(url).await?;
        Ok(cookies_from(&resp))
    }
}

pub fn cookies_from(resp: &Response) -> String {
    let mut jar: Vec<(String, String)> = Vec::new();
    for raw in resp.headers().get_all(SET_COOKIE).iter() {
        let line = raw.to_str().unwrap_or("");
        let pair = line.split(';').next().unwrap_or("");
        if let Some(pos) = pair.find('=') {
            let k = pair[..pos].trim().to_string();
            let v = pair[pos + 1..].trim().to_string();
            if k.is_empty() {
                continue;
            }
            if let Some(existing) = jar.iter_mut().find(|(key, _)| key == &k) {
                existing.1 = v;
            } else {
                jar.push((k, v));
            }
        }
    }
    jar.into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}
