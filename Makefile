.PHONY: check fmt fmt-check lint test test-all test-unit test-tui test-e2e ci run screenshots doctor deps release-assets nextest watch build build-intel build-arm build-local-macos coverage coverage-check release clean rebuild install brew-install

# Prefer cargo-nextest when available; fall back to cargo test for bare clones.
# S2 split targets (test-unit/test-tui/test-e2e) layer on top via NEXTEST_FILTER.
NEXTTEST := $(shell command -v cargo-nextest 2>/dev/null)

# Print a one-line choice so logs show which runner a target used.
ifeq ($(NEXTTEST),)
TEST_RUNNER = cargo test
NEXTEST_HINT_MSG = "Optional: cargo install cargo-nextest  # faster, per-test isolation"
else
TEST_RUNNER = cargo nextest run
NEXTEST_HINT_MSG =
endif

check:
	cargo check --all --all-features

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint: fmt-check
	cargo clippy --all-targets --all-features -- -D warnings

# S2 — split targets.
# test-unit = andre-core (lib + bin unit tests) + andre (lib + bin unit tests).
#            Excludes tui_e2e and andre-core e2e/integration test binaries.
# test-tui  = interaction + render + status_cache + config_* suites (fast, no real stow).
# test-e2e  = tui_e2e + andre-core tests/e2e.rs + tests/integration.rs (real stow + binary).
# S1 — `test` and `ci` use the nextest-preferring runner.
#
# IMPORTANT: nextest sets `CARGO_BIN_EXE_<name>` env vars per test process, so
# process-level smoke tests in `andre/tests/smoke.rs` (M09) work under both
# cargo test and cargo nextest without per-test path guessing.

test-unit:
	@echo "Using runner: $(TEST_RUNNER) (filter: test-unit)"
	@if [ -n "$(NEXTTEST)" ]; then \
		$(TEST_RUNNER) --workspace --all-features -E 'kind(lib) | kind(bin)'; \
	else \
		$(TEST_RUNNER) --lib --bins --all-features; \
	fi

test-tui:
	@echo "Using runner: $(TEST_RUNNER) (filter: test-tui)"
	@if [ -n "$(NEXTTEST)" ]; then \
		$(TEST_RUNNER) --workspace --all-features -E 'binary(/^(interaction|render|status_cache|config_discovery|config_persistence|smoke)/)'; \
	else \
		$(TEST_RUNNER) --test interaction --test render --test status_cache --test config_discovery --test config_persistence --test smoke --all-features; \
	fi

test-e2e:
	@echo "Using runner: $(TEST_RUNNER) (filter: test-e2e)"
	@if [ -n "$(NEXTTEST)" ]; then \
		$(TEST_RUNNER) --workspace --all-features -E 'binary(/^(tui_e2e|e2e|integration|helpers)/)'; \
	else \
		$(TEST_RUNNER) --test tui_e2e --test e2e --test integration --test helpers --all-features; \
	fi

# Full suite via the nextest-preferring runner. lint already runs in `make test`.
test: lint test-unit test-tui test-e2e

test-all: test
	@echo 'All tests passed'

ci: fmt-check
	cargo clippy --all-targets --all-features -- -D warnings
	@if [ -n "$(NEXTTEST)" ]; then \
		$(TEST_RUNNER) --workspace --all-features --locked --profile ci; \
	else \
		$(TEST_RUNNER) --all --all-features --locked; \
	fi

run:
	@mkdir -p demo/target
	cargo run -p andre -- -c demo/andre.yml

screenshots:
	@echo 'Launching demo fixture — capture frames into docs/screenshots/'
	@echo 'Expected names: main-menu.png group-select.png package-select.png settings.png status.png execute.png'
	@mkdir -p demo/target
	cargo run -p andre -- -c demo/andre.yml

doctor:
	./check_dependencies.sh

deps:
	cargo fetch --locked
	@command -v cargo-llvm-cov >/dev/null 2>&1 || echo 'Optional: cargo install cargo-llvm-cov  # for make coverage / coverage-check'
	@command -v cargo-nextest >/dev/null 2>&1 || echo 'Optional: cargo install cargo-nextest  # for make test (default in CI) and make nextest'
	@command -v cargo-watch >/dev/null 2>&1 || echo 'Optional: cargo install cargo-watch  # for make watch'

release-assets: build
	@case $$(uname -s) in \
		Darwin) $(MAKE) build-local-macos ;; \
	esac
	@echo 'Release assets in dist/:'
	@ls -la dist/

# Always requires cargo-nextest (unlike make test, which falls back).
# Print install hint when missing so CI logs make the requirement obvious.
nextest:
	@if command -v cargo-nextest >/dev/null 2>&1; then \
		cargo nextest run --all-features; \
	else \
		echo 'cargo-nextest not found.' >&2; \
		echo 'Install with: cargo install cargo-nextest --locked' >&2; \
		exit 1; \
	fi

watch:
	cargo watch -x 'test --all --all-features'

coverage:
	cargo llvm-cov --workspace --html --output-dir target/coverage
	@echo 'Open target/coverage/html/index.html'

coverage-check:
	cargo llvm-cov report --json 2>/dev/null | python3 scripts/coverage-check.py
# coverage thresholds enforced by scripts/coverage-check.py
# (andre-core >= 90%, andre >= 38%; M09 S5 raised andre-core 88 -> 90)

build:
	cargo build --release
	@mkdir -p dist
	@cp target/release/andre dist/andre

build-intel:
	cargo build --release --target x86_64-unknown-linux-gnu
	@mkdir -p dist
	@cp target/x86_64-unknown-linux-gnu/release/andre dist/andre-linux-x86_64

build-arm:
	cargo build --release --target aarch64-unknown-linux-gnu || (echo 'Cross-compilation not available. On x86_64, install: rustup target add aarch64-unknown-linux-gnu' && exit 1)
	@mkdir -p dist
	@cp target/aarch64-unknown-linux-gnu/release/andre dist/andre-linux-arm64

build-local-macos:
	@case $$(uname -m) in \
		x86_64) TARGET=x86_64-apple-darwin ;; \
		arm64) TARGET=aarch64-apple-darwin ;; \
		*) echo 'Unknown architecture' && exit 1 ;; \
	esac; \
	cargo build --release --target $$TARGET; \
	mkdir -p dist; \
	cp target/$$TARGET/release/andre dist/andre-macos-$$(uname -m)

release: lint test build
	@echo 'Build ready for release'

install: build
	sudo cp dist/andre /usr/local/bin/andre

brew-install: build-local-macos
	@case $$(uname -m) in \
		x86_64) PREFIX=/usr/local ;; \
		arm64)  PREFIX=/opt/homebrew ;; \
		*) echo 'Unknown architecture' && exit 1 ;; \
	esac; \
	mkdir -p $$PREFIX/bin; \
	cp dist/andre-macos-$$(uname -m) $$PREFIX/bin/andre; \
	echo "Installed to $$PREFIX/bin/andre"

clean:
	cargo clean
	rm -rf dist

rebuild: clean build
