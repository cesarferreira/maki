.PHONY: build test clean install release lint fmt check docs

# Build the project in debug mode
build:
	cargo build

# Build optimized release binary
release:
	cargo build --release

# Run all tests
test:
	cargo test

# Run tests with output
test-verbose:
	cargo test -- --nocapture

# Clean build artifacts
clean:
	cargo clean

# Install binary to system
install: release
	cargo install --path .

# Uninstall binary
uninstall:
	cargo uninstall maki

# Run clippy linter
lint:
	cargo clippy -- -D warnings

# Format code
fmt:
	cargo fmt

# Check formatting without modifying
fmt-check:
	cargo fmt -- --check

# Run all checks (format, lint, test)
check: fmt-check lint test

# Generate documentation
docs:
	cargo doc --open

# Run the application
run:
	cargo run

# Run with arguments (usage: make run-args ARGS="list")
run-args:
	cargo run -- $(ARGS)

# Watch for changes and rebuild
watch:
	cargo watch -x build

# Bump version (usage: make bump V=patch|minor|major)
bump:
	cargo set-version --bump $(V)

# Publish to crates.io
publish: check
	cargo publish
