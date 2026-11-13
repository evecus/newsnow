.PHONY: sync-web build release run check clean

# 从 ../newsnow-web 重新构建前端并同步到 web/（随后需重新 release 才会嵌入二进制）
sync-web:
	bash scripts/sync-web.sh

check:
	cargo check

build:
	cargo build

release:
	cargo build --release

run:
	RUST_LOG=newsnow_server=info cargo run

clean:
	cargo clean
	rm -f data/cache.db data/cache.db-shm data/cache.db-wal
