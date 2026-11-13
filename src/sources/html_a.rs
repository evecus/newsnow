use anyhow::Result;
use regex::Regex;
use serde_json::json;

use crate::ctx::Ctx;
use crate::model::NewsItem;
use crate::util::date::{parse_relative, today_slash};
use crate::util::html as h;
use crate::util::now_ms;

fn pub_ts(n: &NewsItem) -> i64 {
    n.pub_date
        .as_ref()
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

// ---------------------------------------------------------------- 36氪
pub async fn kr_quick(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx.get_text("https://www.36kr.com/newsflashes").await?;
    let doc = h::parse(&html);
    let items = h::sel(".newsflash-item");
    let a_sel = h::sel("a.item-title");
    let time_sel = h::sel(".time");
    let mut out = Vec::new();
    for el in doc.select(&items) {
        let Some(a) = h::find(el, &a_sel) else { continue };
        let Some(url) = h::attr(a, "href") else { continue };
        let title = h::txt(a);
        let date_str = h::find_txt(el, &time_sel);
        if title.is_empty() || date_str.is_empty() {
            continue;
        }
        out.push(
            NewsItem::new(
                url.clone(),
                title,
                format!("https://www.36kr.com{url}"),
            )
            .extra(json!({ "date": parse_relative(&date_str, "Asia/Shanghai") })),
        );
    }
    Ok(out)
}

/// 同步解析：scraper::Html 非 Send，不能跨 await 持有，故单独拆成普通函数。
fn parse_renqi(html: &str) -> Vec<NewsItem> {
    let doc = h::parse(html);
    let items = h::sel(".article-item-info");
    let a_sel = h::sel("a.article-item-title.weight-bold");
    let desc_sel = h::sel("a.article-item-description.ellipsis-2");
    let author_sel = h::sel(".kr-flow-bar-author");
    let hot_sel = h::sel(".kr-flow-bar-hot span");
    let mut out = Vec::new();
    for el in doc.select(&items) {
        let Some(a) = h::find(el, &a_sel) else { continue };
        let Some(href) = h::attr(a, "href") else { continue };
        let title = h::txt(a);
        if title.is_empty() {
            continue;
        }
        let full = if href.starts_with("http") {
            href.clone()
        } else {
            format!("https://36kr.com{href}")
        };
        let id: String = href.chars().skip(3).collect();
        out.push(
            NewsItem::new(id, title, full).extra(json!({
                "info": format!("{}  |  {}", h::find_txt(el, &author_sel), h::find_txt(el, &hot_sel)),
                "hover": h::find_txt(el, &desc_sel),
            })),
        );
    }
    out
}

pub async fn kr_renqi(ctx: Ctx, now: i64) -> Result<Vec<NewsItem>> {
    let url = format!("https://36kr.com/hot-list/renqi/{}/1", today_slash());
    let html = ctx
        .get_text_h(
            &url,
            &[
                ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36"),
                ("Referer", "https://www.freebuf.com/"),
                ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"),
            ],
        )
        .await?;
    let out = parse_renqi(&html);
    if out.is_empty() {
        return kr_quick(ctx.clone(), now).await;
    }
    Ok(out)
}

// ---------------------------------------------------------------- 虫部落
pub async fn chongbuluo_hot(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let base = "https://www.chongbuluo.com/";
    let html = ctx.get_text(&format!("{base}forum.php?mod=guide&view=hot")).await?;
    let doc = h::parse(&html);
    let rows = h::sel(".bmw table tr");
    let xst = h::sel(".common .xst");
    let link = h::sel(".common a");
    let mut out = Vec::new();
    for el in doc.select(&rows) {
        let title = h::find_txt(el, &xst);
        let Some(path) = h::find_attr(el, &link, "href") else { continue };
        let url = format!("{base}{path}");
        out.push(
            NewsItem::new(url.clone(), title.clone(), url)
                .extra(json!({ "hover": title })),
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------- FastBull
pub async fn fastbull_express(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let base = "https://www.fastbull.com";
    let html = ctx.get_text(&format!("{base}/cn/express-news")).await?;
    let doc = h::parse(&html);
    let list = h::sel(".content-list.news-list");
    let title_sel = h::sel(".title_name");
    let href_sel = h::sel("[data-href]");
    let id_sel = h::sel("[data-id]");
    let bracket = Regex::new(r"【(.+)】").unwrap();
    let mut out = Vec::new();
    for el in doc.select(&list) {
        let raw_title = h::find_txt(el, &title_sel);
        let url = h::find_attr(el, &href_sel, "data-href")
            .or_else(|| h::find_attr(el, &id_sel, "data-id"));
        let date = h::attr(el, "data-date");
        let (Some(url), Some(date)) = (url, date) else { continue };
        if raw_title.is_empty() {
            continue;
        }
        let short = bracket
            .captures(&raw_title)
            .map(|c| c[1].to_string())
            .unwrap_or_else(|| raw_title.clone());
        let title = if short.chars().count() < 4 { raw_title } else { short };
        let path = if url.starts_with('/') {
            url.clone()
        } else {
            format!("/cn/fastshort/{url}")
        };
        out.push(
            NewsItem::new(url.clone(), title, format!("{base}{path}"))
                .pubv(date.parse::<i64>().unwrap_or(0)),
        );
    }
    out.sort_by(|a, b| pub_ts(b).cmp(&pub_ts(a)));
    Ok(out)
}

pub async fn fastbull_news(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let base = "https://www.fastbull.com";
    let html = ctx.get_text(&format!("{base}/cn/news")).await?;
    let doc = h::parse(&html);
    let list = h::sel(".trending_type");
    let title_sel = h::sel(".title");
    let date_sel = h::sel("[data-date]");
    let mut out = Vec::new();
    for el in doc.select(&list) {
        let Some(url) = h::attr(el, "href") else { continue };
        let title = h::find_txt(el, &title_sel);
        let date = h::find_attr(el, &date_sel, "data-date");
        let (Some(date), false) = (date, title.is_empty()) else { continue };
        let full = if url.starts_with("http") {
            url.clone()
        } else {
            format!("{base}{url}")
        };
        out.push(NewsItem::new(url, title, full).pubv(date.parse::<i64>().unwrap_or(0)));
    }
    out.sort_by(|a, b| pub_ts(b).cmp(&pub_ts(a)));
    Ok(out)
}

// ---------------------------------------------------------------- 格隆汇
pub async fn gelonghui(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let base = "https://www.gelonghui.com";
    let html = ctx.get_text(&format!("{base}/news/")).await?;
    let doc = h::parse(&html);
    let list = h::sel(".article-content");
    let a_sel = h::sel(".detail-right>a");
    let h2 = h::sel("h2");
    let info_sel = h::sel(".time > span:nth-child(1)");
    let rel_sel = h::sel(".time > span:nth-child(3)");
    let mut out = Vec::new();
    for el in doc.select(&list) {
        let Some(a) = h::find(el, &a_sel) else { continue };
        let Some(url) = h::attr(a, "href") else { continue };
        let title = h::find_txt(a, &h2);
        let rel = h::find_txt(el, &rel_sel);
        if title.is_empty() || rel.is_empty() {
            continue;
        }
        out.push(
            NewsItem::new(url.clone(), title, format!("{base}{url}")).extra(json!({
                "date": parse_relative(&rel, "Asia/Shanghai"),
                "info": h::find_txt(el, &info_sel),
            })),
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------- 果核剥壳
fn ghxi_relative(s: &str) -> i64 {
    let re = Regex::new(r"^(\d+)\s*([秒天周月年]|分钟|小时)").unwrap();
    let Some(c) = re.captures(s.trim()) else { return 0 };
    let n: i64 = c[1].parse().unwrap_or(0);
    let unit: i64 = match &c[2] {
        "秒" => 1000,
        "分钟" => 60_000,
        "小时" => 3_600_000,
        "天" => 86_400_000,
        "周" => 604_800_000,
        "月" => 2_592_000_000,
        "年" => 31_536_000_000,
        _ => 0,
    };
    now_ms() - n * unit
}

pub async fn ghxi(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx.get_text("https://www.ghxi.com/category/all").await?;
    let doc = h::parse(&html);
    let list = h::sel(".sec-panel .sec-panel-body .post-loop li");
    let title_sel = h::sel(".item-content .item-title");
    let desc_sel = h::sel(".item-content .item-excerpt");
    let date_sel = h::sel(".item-content .date");
    let link_sel = h::sel(".item-content .item-title a");
    let mut out = Vec::new();
    for el in doc.select(&list) {
        let Some(url) = h::find_attr(el, &link_sel, "href") else { continue };
        let title = h::find_txt(el, &title_sel).replace('\'', "''");
        let desc = h::find_txt(el, &desc_sel).replace('\'', "''");
        let date = h::find_txt(el, &date_sel);
        out.push(
            NewsItem::new(url.clone(), title, url).extra(json!({
                "hover": desc,
                "date": ghxi_relative(&date),
            })),
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------- GitHub Trending
pub async fn github_trending(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx
        .get_text("https://github.com/trending?spoken_language_code=")
        .await?;
    let doc = h::parse(&html);
    let list = h::sel("main .Box div[data-hpc] > article");
    let a_sel = h::sel("h2 > a");
    let star_sel = h::sel("[href$=stargazers]");
    let desc_sel = h::sel("p");
    let mut out = Vec::new();
    for el in doc.select(&list) {
        let Some(a) = h::find(el, &a_sel) else { continue };
        let Some(url) = h::attr(a, "href") else { continue };
        let title = h::txt(a).replace('\n', "");
        if title.is_empty() {
            continue;
        }
        let star = h::find_txt(el, &star_sel)
            .split_whitespace()
            .collect::<String>();
        let desc = h::find_txt(el, &desc_sel).replace('\n', "");
        out.push(
            NewsItem::new(url.clone(), title, format!("https://github.com{url}"))
                .extra(json!({ "info": format!("✰ {star}"), "hover": desc })),
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------- Hacker News
async fn hackernews_direct(ctx: &Ctx) -> Result<Vec<NewsItem>> {
    let base = "https://news.ycombinator.com";
    let html = ctx.get_text(base).await?;
    let doc = h::parse(&html);
    let list = h::sel(".athing");
    let link_sel = h::sel(".titleline a");
    let mut out = Vec::new();
    for el in doc.select(&list) {
        let Some(id) = h::attr(el, "id") else { continue };
        let Some(a) = h::find(el, &link_sel) else { continue };
        let title = h::txt(a);
        if title.is_empty() {
            continue;
        }
        let score_sel = h::sel(&format!("#score_{id}"));
        let score = doc.select(&score_sel).next().map(h::txt).unwrap_or_default();
        out.push(
            NewsItem::new(id.clone(), title, format!("{base}/item?id={id}"))
                .extra(json!({ "info": score })),
        );
    }
    Ok(out)
}

pub async fn hackernews(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let direct = hackernews_direct(&ctx).await.unwrap_or_default();
    if !direct.is_empty() {
        return Ok(direct);
    }
    crate::sources::feeds::hackernews_rss(&ctx).await
}

// ---------------------------------------------------------------- 虎扑
pub async fn hupu(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx.get_text("https://bbs.hupu.com/topic-daily-hot").await?;
    let re = Regex::new(
        r#"<li class="bbs-sl-web-post-body">[\s\S]*?<a href="(/[^"]+?\.html)"[^>]*?class="p-title"[^>]*>([^<]+)</a>"#,
    )
    .unwrap();
    Ok(re
        .captures_iter(&html)
        .map(|c| {
            let path = c[1].to_string();
            let url = format!("https://bbs.hupu.com{path}");
            NewsItem::new(path, c[2].trim().to_string(), url.clone()).mobile(url)
        })
        .collect())
}

// ---------------------------------------------------------------- IT之家
pub async fn ithome(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx.get_text("https://www.ithome.com/list/").await?;
    let doc = h::parse(&html);
    let list = h::sel("#list > div.fl > ul > li");
    let a_sel = h::sel("a.t");
    let date_sel = h::sel("i");
    let ads = ["神券", "优惠", "补贴", "京东"];
    let mut out = Vec::new();
    for el in doc.select(&list) {
        let Some(a) = h::find(el, &a_sel) else { continue };
        let Some(url) = h::attr(a, "href") else { continue };
        let title = h::txt(a);
        let date = h::find_txt(el, &date_sel);
        if title.is_empty() || date.is_empty() {
            continue;
        }
        if url.contains("lapin") || ads.iter().any(|k| title.contains(k)) {
            continue;
        }
        out.push(
            NewsItem::new(url.clone(), title, url)
                .pubv(parse_relative(&date, "Asia/Shanghai")),
        );
    }
    out.sort_by(|a, b| pub_ts(b).cmp(&pub_ts(a)));
    Ok(out)
}
