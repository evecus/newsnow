use anyhow::Result;
use regex::Regex;
use serde_json::{json, Value};

use crate::ctx::Ctx;
use crate::model::NewsItem;
use crate::util::date::{to_utc, today_slash};
use crate::util::{g, ga, gi, gid, gs};

fn flag(v: &Value, path: &str) -> bool {
    g(v, path).and_then(|x| x.as_bool()).unwrap_or(false)
}

// ---------------------------------------------------------------- Linux.do
async fn linuxdo(ctx: &Ctx, url: &str, latest: bool) -> Result<Vec<NewsItem>> {
    let v: Value = ctx.get_json(url).await?;
    Ok(ga(&v, "topic_list.topics")
        .iter()
        .filter(|k| flag(k, "visible") && !flag(k, "archived") && !flag(k, "pinned"))
        .map(|k| {
            let id = gi(k, "id");
            let mut n = NewsItem::new(id, gs(k, "title"), format!("https://linux.do/t/topic/{id}"));
            if latest {
                n = n.pubv(to_utc(&gs(k, "created_at"), "UTC").unwrap_or(0));
            }
            n
        })
        .collect())
}

pub async fn linuxdo_hot(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    linuxdo(&ctx, "https://linux.do/top/daily.json", false).await
}

pub async fn linuxdo_latest(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    linuxdo(&ctx, "https://linux.do/latest.json?order=created", true).await
}

// ---------------------------------------------------------------- MKTNews
pub async fn mktnews(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json_h(
            "https://api.mktnews.net/api/flash?type=0&limit=50",
            &[
                ("Origin", "https://mktnews.net"),
                ("Referer", "https://mktnews.net/"),
            ],
        )
        .await?;
    let mut items: Vec<&Value> = ga(&v, "data").iter().collect();
    items.sort_by(|a, b| {
        to_utc(&gs(b, "time"), "UTC")
            .unwrap_or(0)
            .cmp(&to_utc(&gs(a, "time"), "UTC").unwrap_or(0))
    });
    let bracket = Regex::new(r"^【([^】]*)】(.*)$").unwrap();
    Ok(items
        .into_iter()
        .map(|k| {
            let id = gs(k, "id");
            let content = gs(k, "data.content");
            let title = {
                let t = gs(k, "data.title");
                if !t.is_empty() {
                    t
                } else {
                    bracket
                        .captures(&content)
                        .map(|c| c[1].to_string())
                        .unwrap_or_else(|| content.clone())
                }
            };
            let info = if gi(k, "important") == 1 {
                json!("Important")
            } else {
                json!(false)
            };
            NewsItem::new(
                id.clone(),
                title,
                format!("https://mktnews.net/flashDetail.html?id={id}"),
            )
            .pubv(gs(k, "time"))
            .extra(json!({ "info": info, "hover": content }))
        })
        .collect())
}

// ---------------------------------------------------------------- 牛客
pub async fn nowcoder(ctx: Ctx, now: i64) -> Result<Vec<NewsItem>> {
    let url = format!(
        "https://gw-c.nowcoder.com/api/sparta/hot-search/top-hot-pc?size=20&_={now}&t="
    );
    let v: Value = ctx.get_json(&url).await?;
    Ok(ga(&v, "data.result")
        .iter()
        .map(|k| {
            let (id, url) = match gi(k, "type") {
                74 => (
                    gs(k, "uuid"),
                    format!("https://www.nowcoder.com/feed/main/detail/{}", gs(k, "uuid")),
                ),
                0 => (
                    gs(k, "id"),
                    format!("https://www.nowcoder.com/discuss/{}", gs(k, "id")),
                ),
                _ => (String::new(), String::new()),
            };
            NewsItem::new(id, gs(k, "title"), url)
        })
        .collect())
}

// ---------------------------------------------------------------- 腾讯视频
pub async fn qqvideo(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let body = json!({
        "page_params": {
            "rank_channel_id": "100113",
            "rank_name": "HotSearch",
            "rank_page_size": "30",
            "tab_mvl_sub_mod_id": "792ac_19e77Sub_1b2",
            "tab_name": "热搜榜",
            "tab_type": "hot_rank",
            "tab_vl_data_src": "f5200deb4596bbf3",
            "page_id": "scms_shake",
            "page_type": "scms_shake",
            "source_key": "",
            "tag_id": "",
            "tag_type": "",
            "new_mark_label_enabled": "1"
        },
        "page_context": { "page_index": "1" },
        "flip_info": {
            "page_strategy_id": "",
            "page_module_id": "792ac_19e77",
            "module_strategy_id": {},
            "sub_module_id": "20251106065177",
            "flip_params": {
                "folding_screen_show_num": "",
                "is_mvl": "1",
                "mvl_strategy_info": "{\"default_strategy_id\":\"06755800b45b49238582a6fa1ad0f5c5\",\"default_version\":\"3836\",\"hit_page_uuid\":\"b5080d97dc694a5fb50eb9e7c99326ac\",\"hit_tab_info\":null,\"gray_status_info\":null,\"bypass_to_un_exp_id\":\"\"}",
                "mvl_sub_mod_id": "20251106065177",
                "pad_post_show_num": "",
                "pad_pro_post_show_num": "",
                "pad_pro_small_hor_pic_display_num": "",
                "pad_small_hor_pic_display_num": "",
                "page_id": "scms_shake",
                "page_num": "0",
                "page_type": "scms_shake",
                "post_show_num": "",
                "shake_size": "",
                "small_hor_pic_display_num": "",
                "source_key": "100113",
                "un_policy_id": "06755800b45b49238582a6fa1ad0f5c5",
                "un_strategy_id": "06755800b45b49238582a6fa1ad0f5c5"
            },
            "relace_children_key": []
        }
    });
    let url = "https://pbaccess.video.qq.com/trpc.vector_layout.page_view.PageService/getCard?video_appid=3000010&vversion_platform=2";
    let v: Value = ctx
        .post_json(url, &body, &[("Referer", "https://v.qq.com/")])
        .await?;
    Ok(ga(&v, "data.card.children_list.list.cards")
        .iter()
        .map(|k| {
            let id = gs(k, "id");
            let date = {
                let d = gs(k, "params.publish_date");
                if d.is_empty() { today_slash() } else { d }
            };
            NewsItem::new(
                id.clone(),
                gs(k, "params.title"),
                format!("https://v.qq.com/x/cover/{id}.html"),
            )
            .pubv(date)
            .extra(json!({ "hover": g(k, "params.sub_title") }))
        })
        .collect())
}

// ---------------------------------------------------------------- 少数派
pub async fn sspai(ctx: Ctx, now: i64) -> Result<Vec<NewsItem>> {
    let url = format!("https://sspai.com/api/v1/article/tag/page/get?limit=30&offset=0&created_at={now}&tag=%E7%83%AD%E9%97%A8%E6%96%87%E7%AB%A0&released=false");
    let v: Value = ctx.get_json(&url).await?;
    Ok(ga(&v, "data")
        .iter()
        .map(|k| {
            let id = gi(k, "id");
            NewsItem::new(id, gs(k, "title"), format!("https://sspai.com/post/{id}"))
        })
        .collect())
}

// ---------------------------------------------------------------- 腾讯新闻
pub async fn tencent(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json_h(
            "https://i.news.qq.com/web_backend/v2/getTagInfo?tagId=aEWqxLtdgmQ%3D",
            &[("Referer", "https://news.qq.com/")],
        )
        .await?;
    Ok(ga(&v, "data.tabs.0.articleList")
        .iter()
        .map(|k| {
            NewsItem::new(gid(k, "id"), gs(k, "title"), gs(k, "link_info.url"))
                .extra(json!({ "hover": g(k, "desc") }))
        })
        .collect())
}

// ---------------------------------------------------------------- 澎湃
pub async fn thepaper(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json("https://cache.thepaper.cn/contentapi/wwwIndex/rightSidebar")
        .await?;
    Ok(ga(&v, "data.hotNews")
        .iter()
        .map(|k| {
            let id = gs(k, "contId");
            NewsItem::new(
                id.clone(),
                gs(k, "name"),
                format!("https://www.thepaper.cn/newsDetail_forward_{id}"),
            )
            .mobile(format!("https://m.thepaper.cn/newsDetail_forward_{id}"))
        })
        .collect())
}

// ---------------------------------------------------------------- 贴吧
pub async fn tieba(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json("https://tieba.baidu.com/hottopic/browse/topicList")
        .await?;
    Ok(ga(&v, "data.bang_topic.topic_list")
        .iter()
        .map(|k| NewsItem::new(gs(k, "topic_id"), gs(k, "topic_name"), gs(k, "topic_url")))
        .collect())
}

// ---------------------------------------------------------------- 头条
pub async fn toutiao(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json("https://www.toutiao.com/hot-event/hot-board/?origin=toutiao_pc")
        .await?;
    Ok(ga(&v, "data")
        .iter()
        .map(|k| {
            let id = gs(k, "ClusterIdStr");
            NewsItem::new(
                id.clone(),
                gs(k, "Title"),
                format!("https://www.toutiao.com/trending/{id}/"),
            )
            .extra(json!({ "icon": g(k, "LabelUri.url") }))
        })
        .collect())
}

// ---------------------------------------------------------------- V2EX
pub async fn v2ex(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let urls: Vec<String> = ["create", "ideas", "programmer", "share"]
        .iter()
        .map(|k| format!("https://www.v2ex.com/feed/{k}.json"))
        .collect();
    let list = futures::future::try_join_all(urls.iter().map(|u| ctx.get_json::<Value>(u))).await?;
    let mut out: Vec<NewsItem> = list
        .iter()
        .flat_map(|v| ga(v, "items"))
        .map(|k| {
            let date = {
                let d = gs(k, "date_modified");
                if d.is_empty() { gs(k, "date_published") } else { d }
            };
            NewsItem::new(gs(k, "id"), gs(k, "title"), gs(k, "url"))
                .extra(json!({ "date": date }))
        })
        .collect();
    out.sort_by(|a, b| {
        let da = a
            .extra
            .as_ref()
            .and_then(|e| e.get("date"))
            .map(|x| x.as_str().unwrap_or(""))
            .unwrap_or("");
        let db = b
            .extra
            .as_ref()
            .and_then(|e| e.get("date"))
            .map(|x| x.as_str().unwrap_or(""))
            .unwrap_or("");
        db.cmp(da)
    });
    Ok(out)
}

// ---------------------------------------------------------------- 华尔街见闻
pub async fn wallstreetcn_live(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json("https://api-one.wallstcn.com/apiv1/content/lives?channel=global-channel&limit=30")
        .await?;
    Ok(ga(&v, "data.items")
        .iter()
        .map(|k| {
            let title = {
                let t = gs(k, "title");
                if t.is_empty() { gs(k, "content_text") } else { t }
            };
            NewsItem::new(gid(k, "id"), title, gs(k, "uri"))
                .extra(json!({ "date": gi(k, "display_time") * 1000 }))
        })
        .collect())
}

pub async fn wallstreetcn_news(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json("https://api-one.wallstcn.com/apiv1/content/information-flow?channel=global-channel&accept=article&limit=30")
        .await?;
    Ok(ga(&v, "data.items")
        .iter()
        .filter(|k| {
            let rt = gs(k, "resource_type");
            rt != "theme"
                && rt != "ad"
                && gs(k, "resource.type") != "live"
                && !gs(k, "resource.uri").is_empty()
        })
        .map(|k| {
            let title = {
                let t = gs(k, "resource.title");
                if t.is_empty() { gs(k, "resource.content_short") } else { t }
            };
            NewsItem::new(gid(k, "resource.id"), title, gs(k, "resource.uri"))
                .extra(json!({ "date": gi(k, "resource.display_time") * 1000 }))
        })
        .collect())
}

pub async fn wallstreetcn_hot(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json("https://api-one.wallstcn.com/apiv1/content/articles/hot?period=all")
        .await?;
    Ok(ga(&v, "data.day_items")
        .iter()
        .map(|k| NewsItem::new(gid(k, "id"), gs(k, "title"), gs(k, "uri")))
        .collect())
}

// ---------------------------------------------------------------- 知乎
pub async fn zhihu(ctx: Ctx, _now: i64) -> Result<Vec<NewsItem>> {
    let v: Value = ctx
        .get_json("https://www.zhihu.com/api/v3/feed/topstory/hot-list-web?limit=20&desktop=true")
        .await?;
    let tail = Regex::new(r"(\d+)$").unwrap();
    Ok(ga(&v, "data")
        .iter()
        .map(|k| {
            let url = gs(k, "target.link.url");
            let id = tail
                .captures(&url)
                .map(|c| c[1].to_string())
                .unwrap_or_else(|| url.clone());
            NewsItem::new(id, gs(k, "target.title_area.text"), url).extra(json!({
                "info": g(k, "target.metrics_area.text"),
                "hover": g(k, "target.excerpt_area.text"),
            }))
        })
        .collect())
}
