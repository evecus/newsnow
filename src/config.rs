/// 运行配置：仅来自命令行参数，不读取环境变量。
///
///   ./newsnow -p 0.0.0.0:4444 -db ./data/cache.db -token xxx
pub struct Config {
    /// 监听地址，默认 0.0.0.0:4444
    pub addr: String,
    /// 指定 -db 才启用缓存（SQLite 文件路径）；不指定则不缓存
    pub db_path: Option<String>,
    /// ProductHunt API token；不指定则回退 RSS
    pub token: String,
    /// 是否输出日志，默认关闭
    pub log: bool,
}

const USAGE: &str = "\
newsnow {version}

用法: newsnow [选项]

选项:
  -p, --port <addr>   监听地址，如 0.0.0.0:4444；只写端口(如 8080)则监听 0.0.0.0:<端口>。默认 0.0.0.0:4444
  -db <path>          SQLite 缓存文件路径；指定后启用缓存，不指定则不缓存
  -token <token>      ProductHunt API token；不指定则回退 RSS
  -log                开启日志输出（默认关闭）
  -h, --help          显示帮助
  -V, --version       显示版本
";

impl Config {
    pub fn from_args() -> Self {
        let mut cfg = Self {
            addr: "0.0.0.0:4444".to_string(),
            db_path: None,
            token: String::new(),
            log: false,
        };
        let mut args = std::env::args().skip(1);
        while let Some(a) = args.next() {
            match a.as_str() {
                "-p" | "--port" => {
                    let v = value(&mut args, &a);
                    cfg.addr = if v.contains(':') { v } else { format!("0.0.0.0:{v}") };
                }
                "-db" | "--db" => cfg.db_path = Some(value(&mut args, &a)),
                "-token" | "--token" => cfg.token = value(&mut args, &a),
                "-log" | "--log" => cfg.log = true,
                "-h" | "-help" | "--help" => {
                    print!("{}", USAGE.replace("{version}", env!("CARGO_PKG_VERSION")));
                    std::process::exit(0);
                }
                "-V" | "-version" | "--version" => {
                    println!("newsnow {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                other => die(&format!("未知参数: {other}")),
            }
        }
        cfg
    }
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
    match args.next() {
        Some(v) if !v.is_empty() => v,
        _ => die(&format!("参数 {flag} 缺少值")),
    }
}

fn die(msg: &str) -> ! {
    eprintln!("{msg}\n\n{}", USAGE.replace("{version}", env!("CARGO_PKG_VERSION")));
    std::process::exit(2);
}
