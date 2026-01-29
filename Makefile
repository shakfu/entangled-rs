.PHONY: test build clean check fmt clippy

test:
	cargo test

build:
	cargo build

release:
	cargo build --release

clean:
	cargo clean

check:
	cargo check

fmt:
	cargo fmt

clippy:
	cargo clippy -- -D warnings

all: fmt clippy test build
