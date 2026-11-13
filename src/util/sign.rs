use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use md5::{Digest, Md5};
use rand::Rng;
use sha1::Sha1;

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub fn md5_hex(s: &str) -> String {
    let mut h = Md5::new();
    h.update(s.as_bytes());
    hex(&h.finalize())
}

pub fn sha1_hex(s: &str) -> String {
    let mut h = Sha1::new();
    h.update(s.as_bytes());
    hex(&h.finalize())
}

pub fn base64_std(s: &str) -> String {
    STANDARD.encode(s.as_bytes())
}

/// 模拟 `Math.random().toString(36).substring(2, n)` 的随机 36 进制串。
pub fn rand36(len: usize) -> String {
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARS[rng.gen_range(0..36)] as char)
        .collect()
}

/// 财联社签名：URLSearchParams 排序后拼接 -> SHA1 hex -> 再 MD5 hex。
pub fn cls_query(extra: &[(&str, String)], base: &[(&str, &str)]) -> String {
    let mut pairs: Vec<(String, String)> = base
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .chain(
            extra
                .iter()
                .map(|(k, v)| ((*k).to_string(), v.clone())),
        )
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let qs = pairs
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let sign = md5_hex(&sha1_hex(&qs));
    format!("{}&sign={}", qs, sign)
}

/// 酷安 X-App-Token，对应 coolapk/utils.ts。
pub fn coolapk_app_token(now_sec: i64) -> String {
    let device_id = [8usize, 4, 4, 4, 12]
        .iter()
        .map(|n| rand36(*n))
        .collect::<Vec<_>>()
        .join("-");
    let hex_now = format!("0x{:x}", now_sec);
    let md5_now = md5_hex(&now_sec.to_string());
    let s = format!(
        "token://com.coolapk.market/c67ef5943784d09750dcfbb31020f0ab?{}${}&com.coolapk.market",
        md5_now, device_id
    );
    let md5_s = md5_hex(&base64_std(&s));
    format!("{}{}{}", md5_s, device_id, hex_now)
}
