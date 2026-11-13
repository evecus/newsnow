pub struct Config {
    pub host: String,
    pub port: u16,
    pub db_path: String,
    pub enable_cache: bool,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: env_or("HOST", "0.0.0.0"),
            port: env_or("PORT", "4444").parse().unwrap_or(4444),
            db_path: env_or("CACHE_DB_PATH", "./data/cache.db"),
            enable_cache: env_or("ENABLE_CACHE", "true") != "false",
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}
