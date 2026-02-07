.PHONY: test build clean check fmt clippy release all pyentangled

# Default targets use default-members (entangled + entangled-cli).
# pyentangled requires maturin: cd pyentangled && maturin develop

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
	cargo fmt --all

clippy:
	cargo clippy -- -D warnings

pyentangled:
	cd pyentangled && maturin develop --release

all: fmt clippy test build
