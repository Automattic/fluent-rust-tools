.PHONY: all build test unit-test cli-test clean help

# Common docker options
rust_docker_container := public.ecr.aws/docker/library/rust:1.86.0
docker_container_repo_dir := /app
rust_docker_run := docker run --rm -v $(PWD):$(docker_container_repo_dir) -w $(docker_container_repo_dir) -e CARGO_HOME=$(docker_container_repo_dir)/.cargo $(rust_docker_container)

# Default target
all: build test

# Build the Rust CLI project natively (without Docker)
build_native:
	@echo "📦 Building Rust CLI project natively..."
	@if ! command -v cargo >/dev/null 2>&1; then \
		echo "❌ Cargo (Rust) is not available in PATH."; \
		echo "   Please install Rust from https://rustup.rs/"; \
		exit 1; \
	fi
	cargo build --release

# Build the Rust CLI project in release mode
build:
	@echo "📦 Building Rust CLI project with Docker..."
	$(rust_docker_run) cargo build --release

# Run all tests
test: unit-test cli-test
	@echo ""
	@echo "🎉 All tests passed! The CLI tool is working correctly."
	@echo ""
	@echo "📄 Usage examples:"
	@echo "   Convert Fluent to Android XML: ./target/release/fluent-tools android from-fluent -i input.ftl -o output.xml"
	@echo "   Convert Android XML to Fluent: ./target/release/fluent-tools android to-fluent -i input.xml -o output.ftl"
	@echo "   Convert Fluent to PO: ./target/release/fluent-tools po from-fluent -i input.ftl -o output.po"
	@echo "   Convert PO to Fluent: ./target/release/fluent-tools po to-fluent -i input.po -o output.ftl"

# Run unit tests
unit-test:
	@echo "✅ Running unit tests with Docker..."
	$(rust_docker_run) cargo test -- --nocapture

# Test CLI with conversions for both Android and PO formats
cli-test: build
	@./scripts/cli-test.sh

# Clean up build artifacts and temporary files
clean:
	@echo "🧹 Cleaning up build artifacts..."
	$(rust_docker_run) cargo clean
	rm -f test_android_output.xml test_android_roundtrip.ftl
	rm -f test_po_output.po test_po_roundtrip.ftl

# Show help
help:
	@echo "🧪 fluent-tools Makefile - Docker-based Rust CLI Development"
	@echo ""
	@echo "🐳 Using Docker image: $(rust_docker_container)"
	@echo ""
	@echo "🎯 Available targets:"
	@echo "  all            - Build and run all tests (default)"
	@echo "  build          - Build the Rust CLI project in release mode using Docker"
	@echo "  test           - Run all tests (unit and CLI) using Docker"
	@echo "  unit-test      - Run unit tests only using Docker"
	@echo "  cli-test       - Test CLI with sample data for both Android and PO formats using Docker"
	@echo "  clean          - Clean build artifacts and temporary files using Docker"
	@echo "  help           - Show this help message"
	@echo ""
	@echo "🚀 This tool supports conversions between:"
	@echo "  • Fluent ↔ Android XML strings"
	@echo "  • Fluent ↔ GNU gettext PO files"
	@echo ""
	@echo "📋 Prerequisites:"
	@echo "  • Docker must be installed and running"
	@echo "  • No local Rust installation required!"
	@echo ""
	@echo "🧪 Test scripts:"
	@echo "  • CLI integration tests: ./scripts/cli-test.sh"
	@echo "  • (can be run directly or via 'make cli-test')"
	@echo ""
	@echo "💎 For Ruby gem development, use:"
	@echo "  cd ruby && bundle exec rake -T"
