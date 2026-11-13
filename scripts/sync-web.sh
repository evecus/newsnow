#!/usr/bin/env bash
# 从原前端工程重新构建并同步静态产物 / 源元数据到 Rust 工程。
# 用法: scripts/sync-web.sh [前端工程目录]   默认 ../newsnow-web
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB_SRC="${1:-$ROOT/../newsnow-web}"

echo "[1/3] 构建前端: $WEB_SRC"
cd "$WEB_SRC"
pnpm exec vite build

echo "[2/3] 同步 dist/output/public -> $ROOT/web"
rm -rf "$ROOT/web"
mkdir -p "$ROOT/web"
cp -a dist/output/public/. "$ROOT/web/"

echo "[3/3] 同步 shared/sources.json -> $ROOT/data"
cp shared/sources.json "$ROOT/data/sources.json"

echo "完成。随后重新 cargo build --release 即可把新产物嵌入单二进制。"
