use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::ctx::Ctx;
use crate::model::NewsItem;
use crate::util::sign::md5_hex;

#[derive(Debug, Clone, Default)]
pub struct RssItem {
    pub title: String,
    pub link: String,
    pub guid: String,
    pub id: String,
    pub description: String,
    pub created: String,
    pub content_html: String,
}

fn local_name(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    s.rsplit(':').next().unwrap_or(&s).to_ascii_lowercase()
}

/// 同时兼容 RSS 2.0 `<item>` 与 Atom `<entry>`。
pub fn parse(xml: &str) -> Vec<RssItem> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut cur: Option<RssItem> = None;
    let mut tag: Option<String> = None;
    let mut text = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref());
                if name == "item" || name == "entry" {
                    cur = Some(RssItem::default());
                    tag = None;
                    text.clear();
                } else if cur.is_some() {
                    // Atom 的 <link href="..."/>
                    if name == "link" {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"href" {
                                let href = String::from_utf8_lossy(&a.value).to_string();
                                if let Some(item) = cur.as_mut() {
                                    if item.link.is_empty() {
                                        item.link = href;
                                    }
                                }
                            }
                        }
                    }
                    tag = Some(name);
                    text.clear();
                }
            }
            Ok(Event::Empty(e)) => {
                if cur.is_some() {
                    let name = local_name(e.name().as_ref());
                    if name == "link" {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"href" {
                                let href = String::from_utf8_lossy(&a.value).to_string();
                                if let Some(item) = cur.as_mut() {
                                    if item.link.is_empty() {
                                        item.link = href;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if tag.is_some() {
                    text.push_str(e.unescape().unwrap_or_default().as_ref());
                }
            }
            Ok(Event::CData(e)) => {
                if tag.is_some() {
                    text.push_str(&String::from_utf8_lossy(&e.into_inner()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref());
                if name == "item" || name == "entry" {
                    if let Some(mut item) = cur.take() {
                        if item.link.is_empty() {
                            item.link = item.id.clone();
                        }
                        out.push(item);
                    }
                    tag = None;
                    text.clear();
                    continue;
                }
                if let (Some(item), Some(t)) = (cur.as_mut(), tag.take()) {
                    let val = std::mem::take(&mut text);
                    match t.as_str() {
                        "title" => item.title = val,
                        "link" => {
                            if item.link.is_empty() {
                                item.link = val;
                            }
                        }
                        "guid" => item.guid = val,
                        "id" => item.id = val,
                        "pubdate" | "published" | "updated" | "date" | "created" | "dc:date" => {
                            item.created = val
                        }
                        "description" | "summary" | "encoded" | "content" => {
                            if item.description.is_empty() {
                                item.description = val.clone();
                            }
                            item.content_html = val;
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

pub async fn fetch(ctx: &Ctx, url: &str) -> Result<Vec<RssItem>> {
    let xml = ctx.get_text(url).await?;
    Ok(parse(&xml))
}

/// 对应 defineRSSSource：id 用 link，可选保留 pubDate。
pub fn to_news(items: Vec<RssItem>, hidden_date: bool) -> Vec<NewsItem> {
    items
        .into_iter()
        .filter(|i| !i.link.is_empty() && !i.title.is_empty())
        .map(|i| {
            let mut n = NewsItem::new(i.link.clone(), i.title, i.link);
            if !hidden_date && !i.created.is_empty() {
                n = n.pubv(i.created);
            }
            n
        })
        .collect()
}

/// 对应 defineRSSHubSource 的 JSON 输出（RSSHub ?format=json）。
#[allow(dead_code)]
pub fn rsshub_to_news(v: &serde_json::Value, hidden_date: bool) -> Vec<NewsItem> {
    use crate::util::{ga, gs};
    ga(v, "items")
        .iter()
        .map(|item| {
            let url = gs(item, "url");
            let id = gs(item, "id");
            let date = gs(item, "date_published");
            let mut n = NewsItem::new(
                if id.is_empty() { url.clone() } else { id },
                gs(item, "title"),
                url,
            );
            if !hidden_date && !date.is_empty() {
                n = n.pubv(date);
            }
            n
        })
        .collect()
}

/// RSSHub 里部分源用 md5 作 id；保留工具函数以便扩展。
#[allow(dead_code)]
pub fn md5_id(s: &str) -> String {
    md5_hex(s)
}
