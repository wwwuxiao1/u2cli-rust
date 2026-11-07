#!/bin/sh
# 构建当前平台的单一 u2cli，并放到仓库的 dist/u2cli。
set -eu
cd "$(dirname "$0")/.."
export PATH="${HOME}/.cargo/bin:${PATH}"
# 不用测试正在占用的 target，避免和 cargo test 抢同一把锁。
export CARGO_TARGET_DIR="$(pwd)/target/package"
cargo build --release --offline
mkdir -p dist
cp "${CARGO_TARGET_DIR}/release/u2cli" dist/u2cli
chmod +x dist/u2cli
printf '%s\n' "dist/u2cli"
