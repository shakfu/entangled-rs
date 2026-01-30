.PHONY: test build clean check fmt clippy

test:
	cargo test --workspace

build:
	cargo build --workspace

release:
	cargo build --workspace --release

clean:
	cargo clean

check:
	cargo check --workspace

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace -- -D warnings

all: fmt clippy test build
