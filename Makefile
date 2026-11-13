.PHONY: all frontend check build release run clean

# 一键：构建前端 (frontend/dist) -> 编译 Rust 单二进制（rust-embed 直接嵌入 frontend/dist）
all: release

# 构建前端源码，产物在 frontend/dist
frontend:
	cd frontend && pnpm install && pnpm exec vite build

check: frontend
	cargo check

build: frontend
	cargo build

release: frontend
	cargo build --release

run: frontend
	RUST_LOG=newsnow_server=info cargo run

clean:
	cargo clean
	rm -rf frontend/dist
	rm -f data/cache.db data/cache.db-shm data/cache.db-wal
