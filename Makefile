.PHONY: test build clean check fmt clippy release all pyentangled \
		test-pyentangled test-all install

# Default targets use default-members (entangled + entangled-cli).
# pyentangled requires maturin: cd pyentangled && maturin develop

test:
	@cargo test

build:
	@cargo build

release:
	@cargo build --release

clean:
	@cargo clean

check:
	@cargo check

fmt:
	@cargo fmt --all

clippy:
	@cargo clippy -- -D warnings

pyentangled:
	@cd pyentangled && maturin develop --release

test-pyentangled:
	@cd pyentangled && uv run pytest

test-all: test test-pyentangled

all: fmt clippy test build

install: release
	@echo "copying 'entangled' to ~/.local/bin"
	@cp target/release/entangled ~/.local/bin/