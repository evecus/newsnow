pub mod date;
pub mod html;
pub mod rss;
pub mod sign;

use serde_json::Value;

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// 递归移除对象/数组里的 null，模拟 JS 里 undefined 被 JSON.stringify 忽略的行为。
pub fn compact(v: Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.into_iter()
                .filter(|(_, val)| !val.is_null())
                .map(|(k, val)| (k, compact(val)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.into_iter().map(compact).collect()),
        other => other,
    }
}

pub fn encode_uri_component(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

/// 数字格式化：>=10000 显示 `xw+`，对应 bilibili.ts 的 formatNumber。
pub fn format_number(n: i64) -> String {
    if n >= 10000 {
        format!("{}w+", n / 10000)
    } else {
        n.to_string()
    }
}

/// 按点路径取 JSON 字段，数字段作为数组下标：`g(v, "data.list.0.title")`。
pub fn g<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for seg in path.split('.') {
        if seg.is_empty() {
            continue;
        }
        cur = match seg.parse::<usize>() {
            Ok(idx) => cur.get(idx)?,
            Err(_) => cur.get(seg)?,
        };
    }
    Some(cur)
}

pub fn as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

pub fn gs(v: &Value, path: &str) -> String {
    g(v, path).map(as_text).unwrap_or_default()
}

pub fn gi(v: &Value, path: &str) -> i64 {
    g(v, path)
        .and_then(|x| {
            x.as_i64()
                .or_else(|| x.as_u64().map(|n| n as i64))
                .or_else(|| x.as_f64().map(|n| n as i64))
                .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
        })
        .unwrap_or(0)
}

#[allow(dead_code)]
pub fn gf(v: &Value, path: &str) -> f64 {
    g(v, path).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

#[allow(dead_code)]
pub fn gb(v: &Value, path: &str) -> bool {
    g(v, path)
        .and_then(|x| x.as_bool().or_else(|| x.as_i64().map(|n| n != 0)))
        .unwrap_or(false)
}

/// 取数组切片，永不为 None，方便直接 for 循环。
pub fn ga<'a>(v: &'a Value, path: &str) -> &'a [Value] {
    g(v, path)
        .and_then(|x| x.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

/// id 可能是字符串也可能是数字，原样保留类型。
pub fn gid(v: &Value, path: &str) -> Value {
    match g(v, path) {
        Some(x @ (Value::String(_) | Value::Number(_))) => x.clone(),
        Some(Value::Null) | None => Value::Null,
        Some(x) => x.clone(),
    }
}
