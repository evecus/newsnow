use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::{json, Value};

use crate::ctx::Ctx;
use crate::model::NewsItem;
use crate::util::date::to_utc;
use crate::util::sign::{cls_query, coolapk_app_token};
use crate::util::{as_text, encode_uri_component, format_number, g, ga, gi, gid, gs};

// ---------------------------------------------------------------- 百度热搜
pub async fn baidu(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx.get_text("https://top.baidu.com/board?tab=realtime").await?;
    let re = Regex::new(r"(?s)<!--s-data:(.*?)-->").unwrap();
    let cap = re
        .captures(&html)
        .and_then(|c| c.get(1))
        .ok_or_else(|| anyhow!("baidu: s-data not found"))?;
    let v: Value = serde_json::from_str(cap.as_str())?;
    Ok(ga(&v, "data.cards.0.content")
        .iter()
        .filter(|k| !g(k, "isTop").and_then(|x| x.as_bool()).unwrap_or(false))
        .map(|k| {
            let url = gs(k, "rawUrl");
            NewsItem::new(url.clone(), gs(k, "word"), url).extra(json!({ "hover": g(k, "desc") }))
        })
        .collect())
}

// ---------------------------------------------------------------- B 站
pub async fn bilibili_hot_search(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json("https://s.search.bilibili.com/main/hotword?limit=30")
        .await?;
    Ok(ga(&v, "list")
        .iter()
        .map(|k| {
            let kw = gs(k, "keyword");
            NewsItem::new(
                kw.clone(),
                gs(k, "show_name"),
                format!(
                    "https://search.bilibili.com/all?keyword={}",
                    encode_uri_component(&kw)
                ),
            )
            .extra(json!({ "icon": g(k, "icon") }))
        })
        .collect())
}

async fn bilibili_videos(ctx: &Ctx, url: &str) -> Result<Vec<NewsItem>> {
    let v: Value = ctx.get_json(url).await?;
    Ok(ga(&v, "data.list")
        .iter()
        .map(|k| {
            let bvid = gs(k, "bvid");
            NewsItem::new(
                bvid.clone(),
                gs(k, "title"),
                format!("https://www.bilibili.com/video/{bvid}"),
            )
            .pubv(gi(k, "pubdate") * 1000)
            .extra(json!({
                "info": format!("{} · {}观看 · {}点赞",
                    gs(k, "owner.name"),
                    format_number(gi(k, "stat.view")),
                    format_number(gi(k, "stat.like"))),
                "hover": g(k, "desc"),
                "icon": g(k, "pic"),
            }))
        })
        .collect())
}

pub async fn bilibili_hot_video(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    bilibili_videos(&ctx, "https://api.bilibili.com/x/web-interface/popular").await
}

pub async fn bilibili_ranking(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    bilibili_videos(&ctx, "https://api.bilibili.com/x/web-interface/ranking/v2").await
}

// ---------------------------------------------------------------- 参考消息
pub async fn cankaoxiaoxi(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let urls: Vec<String> = ["zhongguo", "guandian", "gj"]
        .iter()
        .map(|k| format!("http://china.cankaoxiaoxi.com/json/channel/{k}/list.json"))
        .collect();
    let list = futures::future::try_join_all(urls.iter().map(|u| ctx.get_json::<Value>(u))).await?;
    let mut out = Vec::new();
    for v in list {
        for k in ga(&v, "list") {
            let date = to_utc(&gs(k, "data.publishTime"), "Asia/Shanghai").unwrap_or(0);
            out.push(
                NewsItem::new(gid(k, "data.id"), gs(k, "data.title"), gs(k, "data.url"))
                    .extra(json!({ "date": date })),
            );
        }
    }
    out.sort_by(|a, b| extra_date(b).cmp(&extra_date(a)));
    Ok(out)
}

fn extra_date(n: &NewsItem) -> i64 {
    n.extra
        .as_ref()
        .and_then(|e| e.get("date"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0)
}

// ---------------------------------------------------------------- 财联社
fn cls_url(api: &str, extra: &[(&str, String)]) -> String {
    let base = [
        ("appName", "CailianpressWeb"),
        ("os", "web"),
        ("sv", "7.7.5"),
    ];
    format!("{api}?{}", cls_query(extra, &base))
}

fn cls_item(k: &Value, with_pub: bool) -> NewsItem {
    let id = gi(k, "id");
    let title = {
        let t = gs(k, "title");
        if t.is_empty() { gs(k, "brief") } else { t }
    };
    let mut n = NewsItem::new(id, title, format!("https://www.cls.cn/detail/{id}"))
        .mobile(gs(k, "shareurl"));
    if with_pub {
        n = n.pubv(gi(k, "ctime") * 1000);
    }
    n
}

pub async fn cls_telegraph(ctx: Ctx, now: i64) -> Result<Vec<NewsItem>> {
    let extra = vec![
        ("last_time", (now / 1000).to_string()),
        ("refresh_type", "1".to_string()),
        ("rn", "30".to_string()),
    ];
    let url = cls_url("https://www.cls.cn/v1/roll/get_roll_list", &extra);
    let v: Value = ctx
        .get_json_h(&url, &[("Referer", "https://www.cls.cn/telegraph")])
        .await?;
    Ok(ga(&v, "data.roll_data")
        .iter()
        .filter(|k| gi(k, "is_ad") == 0)
        .map(|k| cls_item(k, true))
        .collect())
}

pub async fn cls_depth(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let url = cls_url("https://www.cls.cn/v3/depth/home/assembled/1000", &[]);
    let v: Value = ctx.get_json(&url).await?;
    let mut items: Vec<&Value> = ga(&v, "data.depth_list").iter().collect();
    items.sort_by(|a, b| gi(b, "ctime").cmp(&gi(a, "ctime")));
    Ok(items.into_iter().map(|k| cls_item(k, true)).collect())
}

pub async fn cls_hot(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let url = cls_url("https://www.cls.cn/v2/article/hot/list", &[]);
    let v: Value = ctx.get_json(&url).await?;
    Ok(ga(&v, "data").iter().map(|k| cls_item(k, false)).collect())
}

// ---------------------------------------------------------------- 酷安
pub async fn coolapk(ctx: Ctx, now: i64) -> Result<Vec<NewsItem>> {
    let url = "https://api.coolapk.com/v6/page/dataList?url=%2Ffeed%2FstatList%3FcacheExpires%3D300%26statType%3Dday%26sortField%3Ddetailnum%26title%3D%E4%BB%8A%E6%97%A5%E7%83%AD%E9%97%A8&title=%E4%BB%8A%E6%97%A5%E7%83%AD%E9%97%A8&subTitle=&page=1";
    let token = coolapk_app_token(now / 1000);
    let headers: Vec<(String, String)> = vec![
        ("X-Requested-With".into(), "XMLHttpRequest".into()),
        ("X-App-Id".into(), "com.coolapk.market".into()),
        ("X-App-Token".into(), token),
        ("X-Sdk-Int".into(), "29".into()),
        ("X-Sdk-Locale".into(), "zh-CN".into()),
        ("X-App-Version".into(), "11.0".into()),
        ("X-Api-Version".into(), "11".into()),
        ("X-App-Code".into(), "2101202".into()),
        ("User-Agent".into(), "Dalvik/2.1.0 (Linux; U; Android 10; Redmi K30 5G MIUI/V12.0.3.0.QGICMXM) (#Build; Redmi; Redmi K30 5G; QKQ1.191222.002 test-keys; 10) +CoolMarket/11.0-2101202".into()),
    ];
    let v: Value = ctx.get_json_dyn(url, &headers).await?;
    let list = ga(&v, "data");
    if list.is_empty() {
        return Err(anyhow!("coolapk: empty data"));
    }
    Ok(list
        .iter()
        .filter(|k| !gs(k, "id").is_empty())
        .map(|i| {
            let id = gs(i, "id");
            let title = {
                let t = gs(i, "editor_title");
                if !t.is_empty() {
                    t
                } else {
                    crate::util::html::strip_tags(&gs(i, "message"))
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_string()
                }
            };
            NewsItem::new(
                id,
                title,
                format!("https://www.coolapk.com{}", gs(i, "url")),
            )
            .extra(json!({ "info": g(i, "targetRow.subTitle") }))
        })
        .collect())
}

// ---------------------------------------------------------------- 懂球帝
pub async fn dongqiudi(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json("https://api.dongqiudi.com/app/tabs/web/1.json")
        .await?;
    let mut out: Vec<NewsItem> = ga(&v, "articles")
        .iter()
        .map(|k| {
            let id = gi(k, "id");
            let created = gs(k, "created_at");
            let date = if created.is_empty() {
                0
            } else {
                to_utc(&created, "Asia/Shanghai").unwrap_or(0)
            };
            let url = {
                let s = gs(k, "share");
                if !s.is_empty() {
                    s
                } else {
                    let u = gs(k, "url");
                    if u.is_empty() {
                        format!("https://www.dongqiudi.com/article/{id}")
                    } else {
                        u
                    }
                }
            };
            NewsItem::new(id, gs(k, "title"), url)
                .pubv(created)
                .extra(json!({
                    "icon": g(k, "thumb"),
                    "info": g(k, "category"),
                    "date": date,
                }))
        })
        .collect();
    out.sort_by(|a, b| extra_date(b).cmp(&extra_date(a)));
    Ok(out)
}

// ---------------------------------------------------------------- 豆瓣
pub async fn douban(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json_h(
            "https://m.douban.com/rexxar/api/v2/subject/recent_hot/movie",
            &[
                ("Referer", "https://movie.douban.com/"),
                ("Accept", "application/json, text/plain, */*"),
            ],
        )
        .await?;
    Ok(ga(&v, "items")
        .iter()
        .map(|m| {
            let id = gs(m, "id");
            let sub = gs(m, "card_subtitle");
            let info = sub.split(" / ").take(3).collect::<Vec<_>>().join(" / ");
            NewsItem::new(
                id.clone(),
                gs(m, "title"),
                format!("https://movie.douban.com/subject/{id}"),
            )
            .extra(json!({ "info": info, "hover": sub }))
        })
        .collect())
}

// ---------------------------------------------------------------- 抖音
pub async fn douyin(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let cookie = ctx.cookie_jar("https://login.douyin.com/").await?;
    let v: Value = ctx
        .get_json_dyn(
            "https://www.douyin.com/aweme/v1/web/hot/search/list/?device_platform=webapp&aid=6383&channel=channel_pc_web&detail_list=1",
            &[("cookie".into(), cookie)],
        )
        .await?;
    Ok(ga(&v, "data.word_list")
        .iter()
        .map(|k| {
            let id = gs(k, "sentence_id");
            NewsItem::new(id.clone(), gs(k, "word"), format!("https://www.douyin.com/hot/{id}"))
        })
        .collect())
}

// ---------------------------------------------------------------- 凤凰网
pub async fn ifeng(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx.get_text("https://www.ifeng.com/").await?;
    let re = Regex::new(r"var\s+allData\s*=\s*(\{[\s\S]*?\});").unwrap();
    let Some(cap) = re.captures(&html).and_then(|c| c.get(1)) else {
        return Ok(Vec::new());
    };
    let v: Value = serde_json::from_str(cap.as_str())?;
    Ok(ga(&v, "hotNews1")
        .iter()
        .map(|k| {
            let url = gs(k, "url");
            NewsItem::new(url.clone(), gs(k, "title"), url)
                .extra(json!({ "date": g(k, "newsTime") }))
        })
        .collect())
}

// ---------------------------------------------------------------- 爱奇艺
pub async fn iqiyi(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let url = "https://mesh.if.iqiyi.com/portal/lw/v7/channel/card/videoTab?channelName=recommend&data_source=v7_rec_sec_hot_rank_list&tempId=85&count=30&block_id=hot_ranklist&device=14a4b5ba98e790dce6dc07482447cf48&from=webapp";
    let v: Value = ctx.get_json_h(url, &[("Referer", "https://www.iqiyi.com")]).await?;
    Ok(ga(&v, "items.0.video.0.data")
        .iter()
        .map(|k| {
            NewsItem::new(gid(k, "entity_id"), gs(k, "title"), gs(k, "page_url"))
                .pubv(g(k, "showDate").cloned().unwrap_or(Value::Null))
                .extra(json!({
                    "info": g(k, "desc"),
                    "hover": g(k, "description"),
                    "tag": g(k, "tag"),
                }))
        })
        .collect())
}

// ---------------------------------------------------------------- 金十数据
pub async fn jin10(ctx: Ctx, now: i64) -> Result<Vec<NewsItem>> {
    let raw = ctx
        .get_text(&format!("https://www.jin10.com/flash_newest.js?t={now}"))
        .await?;
    let re = Regex::new(r"^var\s+newest\s*=\s*").unwrap();
    let s = re.replace(&raw, "");
    let s = s.trim().trim_end_matches(';').trim();
    let v: Value = serde_json::from_str(s)?;
    let arr = v.as_array().ok_or_else(|| anyhow!("jin10: not an array"))?;
    let bracket = Regex::new(r"^【([^】]*)】(.*)$").unwrap();
    Ok(arr
        .iter()
        .filter(|k| {
            let has_text = !gs(k, "data.title").is_empty() || !gs(k, "data.content").is_empty();
            let blocked = g(k, "channel")
                .and_then(|x| x.as_array())
                .map(|a| a.iter().any(|x| x.as_i64() == Some(5)))
                .unwrap_or(false);
            has_text && !blocked
        })
        .map(|k| {
            let text = {
                let t = gs(k, "data.title");
                if t.is_empty() { gs(k, "data.content") } else { t }
            };
            let clean = text.replace("<b>", "").replace("</b>", "");
            let (title, desc) = match bracket.captures(&clean) {
                Some(c) => (c[1].to_string(), c[2].to_string()),
                None => (clean, String::new()),
            };
            let id = gs(k, "id");
            let important = gi(k, "important") != 0;
            NewsItem::new(
                id.clone(),
                title,
                format!("https://flash.jin10.com/detail/{id}"),
            )
            .pubv(to_utc(&gs(k, "time"), "Asia/Shanghai").unwrap_or(0))
            .extra(json!({
                "hover": desc,
                "info": if important { json!("✰") } else { json!(false) },
            }))
        })
        .collect())
}

// ---------------------------------------------------------------- 掘金
pub async fn juejin(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json("https://api.juejin.cn/content_api/v1/content/article_rank?category_id=1&type=hot&spider=0")
        .await?;
    Ok(ga(&v, "data")
        .iter()
        .map(|k| {
            let id = gs(k, "content.content_id");
            NewsItem::new(
                id.clone(),
                gs(k, "content.title"),
                format!("https://juejin.cn/post/{id}"),
            )
        })
        .collect())
}

// ---------------------------------------------------------------- 快手
pub async fn kuaishou(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let html = ctx.get_text("https://www.kuaishou.com/?isHome=1").await?;
    let re = Regex::new(r"(?s)window\.__APOLLO_STATE__\s*=\s*(\{.+?\});").unwrap();
    let cap = re
        .captures(&html)
        .and_then(|c| c.get(1))
        .ok_or_else(|| anyhow!("kuaishou: no APOLLO_STATE"))?;
    let v: Value = serde_json::from_str(cap.as_str())?;
    let client = v.get("defaultClient").cloned().unwrap_or(Value::Null);
    let hot_id = gs(&v, "defaultClient.ROOT_QUERY.visionHotRank({\"page\":\"home\"}).id");
    let items = client
        .get(&hot_id)
        .and_then(|h| h.get("items"))
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(items
        .iter()
        .filter_map(|item| {
            let iid = gs(item, "id");
            let hot = client.get(&iid)?;
            if gs(hot, "tagType") == "置顶" {
                return None;
            }
            let name = gs(hot, "name");
            Some(
                NewsItem::new(
                    iid.replace("VisionHotRankItem:", ""),
                    name.clone(),
                    format!(
                        "https://www.kuaishou.com/search/video?searchKey={}",
                        encode_uri_component(&name)
                    ),
                )
                .extra(json!({ "icon": hot.get("iconUrl") })),
            )
        })
        .collect())
}

// ---------------------------------------------------------------- 雪球
pub async fn xueqiu(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let cookie = ctx.cookie_jar("https://xueqiu.com/hq").await?;
    let v: Value = ctx
        .get_json_dyn(
            "https://stock.xueqiu.com/v5/stock/hot_stock/list.json?size=30&_type=10&type=10",
            &[("cookie".into(), cookie)],
        )
        .await?;
    Ok(ga(&v, "data.items")
        .iter()
        .filter(|k| gi(k, "ad") == 0)
        .map(|k| {
            let code = gs(k, "code");
            let percent = g(k, "percent").map(as_text).unwrap_or_default();
            NewsItem::new(
                code.clone(),
                gs(k, "name"),
                format!("https://xueqiu.com/s/{code}"),
            )
            .extra(json!({ "info": format!("{}% {}", percent, gs(k, "exchange")) }))
        })
        .collect())
}
