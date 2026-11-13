pub mod feeds;
pub mod html_a;
pub mod html_b;
pub mod json_a;
pub mod json_b;

use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

use crate::ctx::Ctx;
use crate::model::NewsItem;

pub type BoxFut = Pin<Box<dyn Future<Output = Result<Vec<NewsItem>>> + Send>>;
pub type Getter = fn(Ctx, i64) -> BoxFut;

macro_rules! reg {
    ($($pat:pat => $f:path),+ $(,)?) => {
        /// 每个 sources.json 里的 id（含 redirect 别名）都必须有对应 getter。
        pub fn lookup(id: &str) -> Option<Getter> {
            let f: Getter = match id {
                $($pat => |ctx, now| Box::pin($f(ctx, now)),)+
                _ => return None,
            };
            Some(f)
        }
    };
}

reg! {
    "v2ex" | "v2ex-share" => json_b::v2ex,
    "zhihu" => json_b::zhihu,
    "weibo" => html_b::weibo,
    "zaobao" => html_b::zaobao,
    "coolapk" => json_a::coolapk,
    "mktnews" | "mktnews-flash" => json_b::mktnews,
    "wallstreetcn" | "wallstreetcn-quick" => json_b::wallstreetcn_live,
    "wallstreetcn-news" => json_b::wallstreetcn_news,
    "wallstreetcn-hot" => json_b::wallstreetcn_hot,
    "36kr" | "36kr-quick" => html_a::kr_quick,
    "36kr-renqi" => html_a::kr_renqi,
    "douyin" => json_a::douyin,
    "hupu" => html_a::hupu,
    "dongqiudi" => json_a::dongqiudi,
    "aihot" => feeds::aihot,
    "tieba" => json_b::tieba,
    "toutiao" => json_b::toutiao,
    "ithome" => html_a::ithome,
    "thepaper" => json_b::thepaper,
    "sputniknewscn" => html_b::sputniknewscn,
    "cankaoxiaoxi" => json_a::cankaoxiaoxi,
    "pcbeta" | "pcbeta-windows11" => feeds::pcbeta_windows11,
    "pcbeta-windows" => feeds::pcbeta_windows,
    "cls" | "cls-telegraph" => json_a::cls_telegraph,
    "cls-depth" => json_a::cls_depth,
    "cls-hot" => json_a::cls_hot,
    "xueqiu" | "xueqiu-hotstock" => json_a::xueqiu,
    "gelonghui" => html_a::gelonghui,
    "fastbull" | "fastbull-express" => html_a::fastbull_express,
    "fastbull-news" => html_a::fastbull_news,
    "solidot" => feeds::solidot,
    "hackernews" => html_a::hackernews,
    "producthunt" => feeds::producthunt,
    "github" | "github-trending-today" => html_a::github_trending,
    "bilibili" | "bilibili-hot-search" => json_a::bilibili_hot_search,
    "bilibili-hot-video" => json_a::bilibili_hot_video,
    "bilibili-ranking" => json_a::bilibili_ranking,
    "kuaishou" => json_a::kuaishou,
    "kaopu" => feeds::kaopu,
    "jin10" => json_a::jin10,
    "baidu" => json_a::baidu,
    "nowcoder" => json_b::nowcoder,
    "sspai" => json_b::sspai,
    "juejin" => json_a::juejin,
    "ifeng" => json_a::ifeng,
    "chongbuluo" | "chongbuluo-latest" => feeds::chongbuluo_latest,
    "chongbuluo-hot" => html_a::chongbuluo_hot,
    "douban" => json_a::douban,
    "steam" => html_b::steam,
    "tencent" | "tencent-hot" => json_b::tencent,
    "freebuf" => feeds::freebuf,
    "qqvideo" | "qqvideo-tv-hotsearch" => json_b::qqvideo,
    "iqiyi" | "iqiyi-hot-ranklist" => json_a::iqiyi,
    // 这些 id 在当前 sources.json 里被 disable，但实现仍保留以便随时启用。
    "linuxdo" | "linuxdo-latest" => json_b::linuxdo_latest,
    "linuxdo-hot" => json_b::linuxdo_hot,
    "ghxi" => html_a::ghxi,
    "smzdm" => html_b::smzdm,
}

pub async fn fetch(ctx: Ctx, id: &str, now: i64) -> Result<Vec<NewsItem>> {
    let getter = lookup(id).ok_or_else(|| anyhow::anyhow!("no getter registered for `{id}`"))?;
    getter(ctx, now).await
}
