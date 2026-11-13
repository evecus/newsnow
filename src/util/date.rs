use chrono::{Datelike, DateTime, Duration as ChronoDuration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

use crate::util::now_ms;

pub fn parse_tz(name: &str) -> Tz {
    name.parse().unwrap_or(chrono_tz::Asia::Shanghai)
}

fn naive_formats() -> &'static [&'static str] {
    &[
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y.%m.%d %H:%M:%S",
        "%Y年%m月%d日 %H:%M",
        "%m-%d %H:%M",
        "%m/%d %H:%M",
    ]
}

fn parse_naive(s: &str) -> Option<NaiveDateTime> {
    for f in naive_formats() {
        if let Ok(d) = NaiveDateTime::parse_from_str(s, f) {
            // `%m-%d %H:%M` 解析出的年份是 0，补当前年
            if d.year() < 1970 {
                let y = Utc::now().year();
                if let Some(fixed) = NaiveDate::from_ymd_opt(y, d.month(), d.day())
                    .and_then(|date| date.and_hms_opt(d.hour(), d.minute(), d.second()))
                {
                    return Some(fixed);
                }
            }
            return Some(d);
        }
    }
    for f in ["%Y-%m-%d", "%Y/%m/%d", "%Y年%m月%d日", "%Y.%m.%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            return d.and_hms_opt(0, 0, 0);
        }
    }
    None
}

/// 对应 server/utils/date.ts 的 tranformToUTC：把无时区时间按指定时区解释成毫秒时间戳。
pub fn to_utc(date: &str, tz: &str) -> Option<i64> {
    let s = date.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    if let Ok(n) = s.parse::<i64>() {
        return Some(if n < 100_000_000_000 { n * 1000 } else { n });
    }
    let naive = parse_naive(s)?;
    let zone = parse_tz(tz);
    zone.from_local_datetime(&naive)
        .single()
        .map(|dt| dt.timestamp_millis())
}

fn dur_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(\d+)(年|years?|个月|月|months?|个?星期|周|weeks?|天|日|days?|小时|时|hours?|hrs?|分钟|分|minutes?|mins?|秒钟|秒|seconds?|secs?)",
        )
        .unwrap()
    })
}

fn clock_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d{1,2}):(\d{2})(?::(\d{2}))?\s*(am|pm)?").unwrap())
}

fn unit_ms(u: &str) -> i64 {
    const S: i64 = 1000;
    const M: i64 = 60 * S;
    const H: i64 = 60 * M;
    const D: i64 = 24 * H;
    match u {
        "年" | "year" | "years" => 365 * D,
        "个月" | "月" | "month" | "months" => 30 * D,
        "个星期" | "星期" | "周" | "week" | "weeks" => 7 * D,
        "天" | "日" | "day" | "days" => D,
        "小时" | "时" | "hour" | "hours" | "hr" | "hrs" => H,
        "分钟" | "分" | "minute" | "minutes" | "min" | "mins" => M,
        "秒钟" | "秒" | "second" | "seconds" | "sec" | "secs" => S,
        _ => 0,
    }
}

fn preprocess(s: &str) -> String {
    s.to_lowercase()
        .replace('几', "3")
        .replace('幾', "3")
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .collect()
}

/// 返回 (基准日期, 剩余时间字符串)。
fn special_day(pre: &str, today: NaiveDate) -> Option<(NaiveDate, String)> {
    let words: &[(&str, i64)] = &[
        ("前天", -2),
        ("昨天", -1),
        ("yesterday", -1),
        ("今天", 0),
        ("今日", 0),
        ("today", 0),
        ("明天", 1),
        ("明日", 1),
        ("tomorrow", 1),
        ("后天", 2),
        ("後天", 2),
    ];
    for (w, offset) in words {
        if let Some(rest) = pre.strip_prefix(w) {
            return Some((today + ChronoDuration::days(*offset), rest.to_string()));
        }
    }
    // 周X / 星期X：取最近的该星期几（含今天）
    let weekday_words: &[(&str, u32)] = &[
        ("周一", 0),
        ("星期一", 0),
        ("周二", 1),
        ("星期二", 1),
        ("周三", 2),
        ("星期三", 2),
        ("周四", 3),
        ("星期四", 3),
        ("周五", 4),
        ("星期五", 4),
        ("周六", 5),
        ("星期六", 5),
        ("周日", 6),
        ("星期天", 6),
        ("星期日", 6),
        ("星期天", 6),
    ];
    for (w, target) in weekday_words {
        if let Some(rest) = pre.strip_prefix(w) {
            let cur = today.weekday().num_days_from_monday();
            let back = (cur as i64 - *target as i64).rem_euclid(7);
            return Some((today - ChronoDuration::days(back), rest.to_string()));
        }
    }
    None
}

fn parse_clock(rest: &str) -> (u32, u32, u32) {
    if let Some(c) = clock_re().captures(rest) {
        let mut h: u32 = c[1].parse().unwrap_or(0);
        let m: u32 = c[2].parse().unwrap_or(0);
        let s: u32 = c.get(3).and_then(|x| x.as_str().parse().ok()).unwrap_or(0);
        if let Some(ap) = c.get(4) {
            if ap.as_str() == "pm" && h < 12 {
                h += 12;
            } else if ap.as_str() == "am" && h == 12 {
                h = 0;
            }
        }
        if h < 24 && m < 60 && s < 60 {
            return (h, m, s);
        }
    }
    (0, 0, 0)
}

/// 返回 (累计毫秒, 是否为「之前」)。
fn duration(pre: &str) -> Option<(i64, bool)> {
    let before = pre.contains('前') || pre.contains("ago");
    let after = pre.contains('后') || pre.contains('後') || pre.starts_with("in");
    if !before && !after {
        return None;
    }
    let mut total = 0i64;
    let mut found = false;
    for c in dur_re().captures_iter(pre) {
        let n: i64 = c[1].parse().unwrap_or(0);
        total += n * unit_ms(&c[2]);
        found = true;
    }
    if !found {
        return None;
    }
    Some((total, before))
}

/// 对应 parseRelativeDate，返回时间戳或原始字符串（与 TS 行为一致）。
pub fn parse_relative(date: &str, tz: &str) -> Value {
    let raw = date.trim();
    if raw.is_empty() {
        return Value::Null;
    }
    if raw == "刚刚" {
        return Value::from(now_ms());
    }
    let pre = preprocess(raw);
    let zone = parse_tz(tz);
    let today = Utc::now().with_timezone(&zone).date_naive();

    if let Some((base, rest)) = special_day(&pre, today) {
        let (h, m, s) = parse_clock(&rest);
        if let Some(naive) = base.and_hms_opt(h, m, s) {
            if let Some(dt) = zone.from_local_datetime(&naive).single() {
                return Value::from(dt.timestamp_millis());
            }
        }
    }

    if let Some((ms, before)) = duration(&pre) {
        return Value::from(if before { now_ms() - ms } else { now_ms() + ms });
    }

    if let Some(t) = to_utc(raw, tz) {
        return Value::from(t);
    }
    Value::String(raw.to_string())
}

pub fn today_slash() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

pub fn today_slash_tz(tz: &str) -> String {
    let zone = parse_tz(tz);
    Utc::now().with_timezone(&zone).format("%Y-%m-%d").to_string()
}
