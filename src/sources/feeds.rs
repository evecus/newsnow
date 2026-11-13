use anyhow::{anyhow, Result};
use chrono::{TimeZone, Utc};
use regex::Regex;
use serde_json::{json, Value};

use crate::ctx::Ctx;
use crate::model::NewsItem;
use crate::util::date::today_slash_tz;
use crate::util::rss as rss_util;

async fn simple_rss(ctx: &Ctx, url: &str) -> Result<Vec<NewsItem>> {
    let items = rss_util::fetch(ctx, url).await?;
    if items.is_empty() {
        return Err(anyhow!("Cannot fetch rss data: {url}"));
    }
    Ok(rss_util::to_news(items, false))
}

// ---------------------------------------------------------------- RSS 简单源
pub async fn aihot(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    simple_rss(
        &ctx,
        "https://aihot.news/feed/all.xml?aihot_actor=8d0ffb92-7f76-45e4-9d1b-8341f0eabf6c",
    )
    .await
}

pub async fn kaopu(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    simple_rss(&ctx, "https://kaopunews.com/feed.xml").await
}

pub async fn solidot(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    simple_rss(&ctx, "https://www.solidot.org/index.rss").await
}

pub async fn chongbuluo_latest(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    simple_rss(
        &ctx,
        "https://www.chongbuluo.com/forum.php?mod=rss&view=newthread",
    )
    .await
}

// ---------------------------------------------------------------- Hacker News RSS 兜底
pub async fn hackernews_rss(ctx: &Ctx) -> Result<Vec<NewsItem>> {
    let items = rss_util::fetch(ctx, "https://hnrss.org/frontpage?count=30").await?;
    if items.is_empty() {
        return Err(anyhow!("Cannot fetch rss data"));
    }
    let comments = Regex::new(r#"Comments URL: <a href="([^"]+)""#).unwrap();
    let id_re = Regex::new(r"id=(\d+)").unwrap();
    let points = Regex::new(r"Points: (\d+)").unwrap();
    Ok(items
        .into_iter()
        .map(|item| {
            let url = comments
                .captures(&item.description)
                .map(|c| c[1].to_string())
                .unwrap_or_else(|| item.link.clone());
            let id = id_re
                .captures(&url)
                .map(|c| c[1].to_string())
                .unwrap_or_else(|| url.clone());
            let info = points
                .captures(&item.description)
                .map(|c| format!("{} points", &c[1]));
            NewsItem::new(id, item.title, url)
                .pubv(item.created)
                .extra(json!({ "info": info }))
        })
        .filter(|n| !n.title.is_empty() && !n.url.is_empty())
        .collect())
}

// ---------------------------------------------------------------- FreeBuf
pub async fn freebuf(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    // 原实现在 myFetch 失败时用 curl 兜底；单二进制不依赖外部命令，改为带 UA 直连 + 失败即报错走缓存。
    let xml = ctx
        .get_text_h(
            "https://www.freebuf.com/feed",
            &[("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")],
        )
        .await
        .unwrap_or_default();
    let news: Vec<NewsItem> = rss_util::parse(&xml)
        .into_iter()
        .filter(|i| !i.title.is_empty() && !i.link.is_empty())
        .map(|i| {
            let id = if i.guid.is_empty() { i.link.clone() } else { i.guid };
            NewsItem::new(id, i.title, i.link.clone())
                .pubv(i.created)
                .extra(json!({ "hover": i.description }))
        })
        .collect();
    // 被 WAF 拦截时返回挑战页，解析为空 -> 抛错让上层回退旧缓存。
    if news.is_empty() {
        return Err(anyhow!("Cannot fetch freebuf feed"));
    }
    Ok(news)
}

// ---------------------------------------------------------------- ProductHunt
const PH_FEED: &str = "https://www.producthunt.com/feed";

pub async fn producthunt(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let token = std::env::var("PRODUCTHUNT_API_TOKEN").unwrap_or_default();
    if token.is_empty() {
        return simple_rss(&ctx, PH_FEED).await;
    }
    let zone = chrono_tz::America::Los_Angeles;
    let today = Utc::now().with_timezone(&zone).date_naive();
    let naive = today.and_hms_opt(0, 0, 0).unwrap();
    let posted_after = zone
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| format!("{}T00:00:00Z", today_slash_tz("America/Los_Angeles")));

    let query = r#"
    query($postedAfter: DateTime!) {
      posts(first: 30, order: RANKING, postedAfter: $postedAfter) {
        edges { node { id name tagline votesCount url slug } }
      }
    }"#;
    let body = json!({ "query": query, "variables": { "postedAfter": posted_after } });
    let headers: Vec<(String, String)> = vec![
        ("Authorization".into(), format!("Bearer {token}")),
        ("Content-Type".into(), "application/json".into()),
        ("Accept".into(), "application/json".into()),
    ];
    let text = ctx
        .post_text_dyn(
            "https://api.producthunt.com/v2/api/graphql",
            body.to_string(),
            &headers,
        )
        .await?;
    let v: Value = serde_json::from_str(&text)?;
    let edges = v
        .get("data")
        .and_then(|d| d.get("posts"))
        .and_then(|p| p.get("edges"))
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();
    let news: Vec<NewsItem> = edges
        .iter()
        .filter_map(|edge| {
            let post = edge.get("node")?;
            let id = crate::util::gs(post, "id");
            let title = crate::util::gs(post, "name");
            if id.is_empty() || title.is_empty() {
                return None;
            }
            let url = {
                let u = crate::util::gs(post, "url");
                if u.is_empty() {
                    format!("https://www.producthunt.com/posts/{}", crate::util::gs(post, "slug"))
                } else {
                    u
                }
            };
            Some(
                NewsItem::new(id, title, url).extra(json!({
                    "info": format!(" △︎ {}", crate::util::gi(post, "votesCount")),
                    "hover": crate::util::gs(post, "tagline"),
                })),
            )
        })
        .collect();
    if news.is_empty() {
        return simple_rss(&ctx, PH_FEED).await;
    }
    Ok(news)
}

// ---------------------------------------------------------------- 远景论坛（PCBeta）
fn form_body(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

async fn pcbeta_get(ctx: &Ctx, url: &str, cookie: &str) -> Result<(String, String)> {
    let resp = ctx
        .send(|c| {
            let mut rb = c.get(url);
            if !cookie.is_empty() {
                rb = rb.header("Cookie", cookie);
            }
            rb
        })
        .await?;
    let new_cookie = crate::ctx::cookies_from(&resp);
    let text = resp.text().await?;
    Ok((
        text,
        if new_cookie.is_empty() {
            cookie.to_string()
        } else {
            new_cookie
        },
    ))
}

async fn pcbeta(ctx: &Ctx, fid: i32) -> Result<Vec<NewsItem>> {
    let base = "https://bbs.pcbeta.com";
    let url = format!("{base}/forum.php?mod=rss&fid={fid}&auth=0");
    let (mut text, mut cookie) = pcbeta_get(ctx, &url, "").await?;
    let challenge_re = Regex::new(r#"data-challenge="([^"]+)""#).unwrap();
    let distance_re = Regex::new(r#"data-distance="([^"]+)""#).unwrap();

    for _ in 0..2 {
        if text.contains("<rss") {
            break;
        }
        let form: Option<Vec<(&str, String)>> = if text.contains("action=environment") {
            Some(vec![
                ("action", "environment".into()),
                ("platform", "MacIntel".into()),
                ("webdriver", "0".into()),
                ("headless", "0".into()),
                ("cdp", "0".into()),
            ])
        } else if text.contains("data-challenge") {
            let challenge = challenge_re.captures(&text).map(|c| c[1].to_string());
            let distance = distance_re.captures(&text).map(|c| c[1].to_string());
            match (challenge, distance) {
                (Some(challenge), Some(distance)) => Some(vec![
                    ("action", "slide".into()),
                    ("distance", distance),
                    ("challenge", challenge),
                    ("return_to", format!("/forum.php?mod=rss&fid={fid}&auth=0")),
                    ("client_platform", "MacIntel".into()),
                    ("screen_resolution", "1920x1080".into()),
                    ("webgl_vendor", "blocked_or_unavailable".into()),
                    ("webgl_renderer", "blocked_or_unavailable".into()),
                ]),
                _ => None,
            }
        } else {
            None
        };
        let Some(pairs) = form else { break };
        let headers: Vec<(String, String)> = vec![
            (
                "Content-Type".into(),
                "application/x-www-form-urlencoded".into(),
            ),
            ("Cookie".into(), cookie.clone()),
        ];
        let _ = ctx
            .post_text_dyn(&format!("{base}/__access_review"), form_body(&pairs), &headers)
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        (text, cookie) = pcbeta_get(ctx, &url, &cookie).await?;
    }

    let news: Vec<NewsItem> = rss_util::parse(&text)
        .into_iter()
        .filter(|i| !i.link.is_empty() && !i.title.is_empty())
        .map(|i| {
            NewsItem::new(i.link.clone(), i.title, i.link.clone())
                .pubv(i.created)
                .extra(json!({ "hover": i.description }))
        })
        .collect();
    if news.is_empty() {
        return Err(anyhow!("Cannot fetch rss data: pcbeta {fid}"));
    }
    Ok(news)
}

pub async fn pcbeta_windows11(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    pcbeta(&ctx, 563).await
}

pub async fn pcbeta_windows(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    pcbeta(&ctx, 521).await
}
