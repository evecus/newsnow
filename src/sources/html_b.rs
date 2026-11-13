use anyhow::Result;
use serde_json::{json, Value};

use crate::ctx::Ctx;
use crate::model::NewsItem;
use crate::util::date::parse_relative;
use crate::util::html as h;
use crate::util::now_ms;

fn pub_ts(n: &NewsItem) -> i64 {
    n.pub_date
        .as_ref()
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

// ---------------------------------------------------------------- 什么值得买
pub async fn smzdm(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx.get_text("https://post.smzdm.com/hot_1/").await?;
    let doc = h::parse(&html);
    let list = h::sel("#feed-main-list .z-feed-title");
    let a_sel = h::sel("a");
    let mut out = Vec::new();
    for el in doc.select(&list) {
        let Some(a) = h::find(el, &a_sel) else { continue };
        let Some(url) = h::attr(a, "href") else { continue };
        out.push(NewsItem::new(url.clone(), h::txt(a), url));
    }
    Ok(out)
}

// ---------------------------------------------------------------- 卫星通讯社
pub async fn sputniknewscn(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx
        .get_text("https://sputniknews.cn/services/widget/lenta/")
        .await?;
    let doc = h::parse(&html);
    let list = h::sel(".lenta__item");
    let a_sel = h::sel("a");
    let title_sel = h::sel(".lenta__item-text");
    let date_sel = h::sel(".lenta__item-date");
    let mut out = Vec::new();
    for el in doc.select(&list) {
        let Some(a) = h::find(el, &a_sel) else { continue };
        let Some(url) = h::attr(a, "href") else { continue };
        let title = h::find_txt(a, &title_sel);
        let date = h::find_attr(a, &date_sel, "data-unixtime");
        let (Some(date), false) = (date, title.is_empty()) else { continue };
        let ts = date.parse::<i64>().unwrap_or(0) * 1000;
        out.push(
            NewsItem::new(url.clone(), title, format!("https://sputniknews.cn{url}"))
                .extra(json!({ "date": ts })),
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------- Steam
pub async fn steam(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx
        .get_text("https://store.steampowered.com/stats/stats/")
        .await?;
    let doc = h::parse(&html);
    let rows = h::sel("#detailStats tr.player_count_row");
    let a_sel = h::sel("a.gameLink");
    let count_sel = h::sel("td:first-child .currentServers");
    let mut out = Vec::new();
    for el in doc.select(&rows) {
        let Some(a) = h::find(el, &a_sel) else { continue };
        let Some(url) = h::attr(a, "href") else { continue };
        let name = h::txt(a);
        let players = h::find_txt(el, &count_sel);
        if name.is_empty() || players.is_empty() {
            continue;
        }
        out.push(
            NewsItem::new(url.clone(), name, url)
                .pubv(now_ms())
                .extra(json!({ "info": players })),
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------- 微博热搜
pub async fn weibo(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let base = "https://s.weibo.com";
    let url = format!("{base}/top/summary?cate=realtimehot");
    let html = ctx
        .get_text_h(
            &url,
            &[
                ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36"),
                ("Cookie", "SUB=_2AkMWIuNSf8NxqwJRmP8dy2rhaoV2ygrEieKgfhKJJRMxHRl-yT9jqk86tRB6PaLNvQZR6zYUcYVT1zSjoSreQHidcUq7"),
                ("referer", "https://s.weibo.com/top/summary?cate=realtimehot"),
            ],
        )
        .await?;
    let doc = h::parse(&html);
    let rows = h::sel("#pl_top_realtimehot table tbody tr");
    let link_sel = h::sel("td.td-02 a");
    let flag_sel = h::sel("td.td-03");
    let mut out = Vec::new();
    for row in doc.select(&rows).skip(1) {
        let Some(link) = row.select(&link_sel).find(|e| {
            h::attr(*e, "href")
                .map(|x| !x.contains("javascript:void(0);"))
                .unwrap_or(false)
        }) else {
            continue;
        };
        let title = h::txt(link);
        let Some(href) = h::attr(link, "href") else { continue };
        if title.is_empty() {
            continue;
        }
        let flag = h::find_txt(row, &flag_sel);
        let icon = match flag.as_str() {
            "新" => json!({ "url": "https://simg.s.weibo.com/moter/flags/1_0.png", "scale": 1.5 }),
            "热" => json!({ "url": "https://simg.s.weibo.com/moter/flags/2_0.png", "scale": 1.5 }),
            "爆" => json!({ "url": "https://simg.s.weibo.com/moter/flags/4_0.png", "scale": 1.5 }),
            _ => Value::Null,
        };
        let full = format!("{base}{href}");
        out.push(
            NewsItem::new(title.clone(), title, full.clone())
                .mobile(full)
                .extra(json!({ "icon": icon })),
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------- 联合早报（早晨报，GBK）
pub async fn zaobao(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let bytes = ctx.get_bytes("https://www.zaochenbao.com/realtime/").await?;
    let (decoded, _, _) = encoding_rs::GBK.decode(&bytes);
    let doc = h::parse(decoded.as_ref());
    let list = h::sel("div.list-block>a.item");
    let eps = h::sel(".eps");
    let date_sel = h::sel(".pdt10");
    let base = "https://www.zaochenbao.com";
    let mut out = Vec::new();
    for el in doc.select(&list) {
        let Some(url) = h::attr(el, "href") else { continue };
        let title = h::find_txt(el, &eps);
        let date = h::find_txt(el, &date_sel).replace("- ", " ");
        if title.is_empty() || date.is_empty() {
            continue;
        }
        out.push(
            NewsItem::new(url.clone(), title, format!("{base}{url}"))
                .pubv(parse_relative(&date, "Asia/Shanghai")),
        );
    }
    out.sort_by(|a, b| pub_ts(b).cmp(&pub_ts(a)));
    Ok(out)
}
