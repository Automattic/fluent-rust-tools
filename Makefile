.PHONY: all build test unit-test cli-test clean help

# Common docker options
rust_docker_container := public.ecr.aws/docker/library/rust:1.86.0
docker_container_repo_dir := /app
rust_docker_run := docker run --rm -v $(PWD):$(docker_container_repo_dir) -w $(docker_container_repo_dir) -e CARGO_HOME=$(docker_container_repo_dir)/.cargo $(rust_docker_container)

# Architecture mapping: platform -> rust target triple
PLATFORM_TARGETS := \
	x86_64-linux:x86_64-unknown-linux-gnu \
	arm64-linux:aarch64-unknown-linux-gnu \
	x86_64-darwin:x86_64-apple-darwin \
	arm64-darwin:aarch64-apple-darwin \
	x86_64-windows:x86_64-pc-windows-gnu \
	arm64-windows:aarch64-pc-windows-gnullvm

# Extract platform names from PLATFORM_TARGETS
PLATFORMS := $(shell for mapping in $(PLATFORM_TARGETS); do echo $${mapping%%:*}; done)

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

# Build the Rust CLI project in release mode (current platform)
build:
	@echo "📦 Building Rust CLI project for current platform..."
	@case "$$(uname -s)" in \
		Darwin) \
			echo "🍎 macOS detected - using native build"; \
			$(MAKE) build_native; \
			;; \
		Linux) \
			platform=""; \
			case "$$(uname -m)" in \
				x86_64|amd64) platform="x86_64-linux" ;; \
				arm64|aarch64) platform="arm64-linux" ;; \
				*) echo "❌ Unsupported Linux architecture"; exit 1 ;; \
			esac; \
			echo "🐧 Linux detected - using Docker build for $$platform"; \
			$(MAKE) build_platform PLATFORM=$$platform; \
			;; \
		*) \
			echo "❌ Unsupported OS - please use build_native or build_platform"; \
			exit 1; \
			;; \
	esac

# Build for a specific platform using Docker cross-compilation
# Usage: make build_platform PLATFORM=x86_64-linux
build_platform:
	@if [ -z "$(PLATFORM)" ]; then \
		echo "❌ PLATFORM variable is required. Available platforms:"; \
		echo "   $(PLATFORMS)"; \
		exit 1; \
	fi
	@target=""; \
	linker_setup=""; \
	for mapping in $(PLATFORM_TARGETS); do \
		platform=$${mapping%%:*}; \
		rust_target=$${mapping##*:}; \
		if [ "$$platform" = "$(PLATFORM)" ]; then \
			target=$$rust_target; \
			case "$$target" in \
				"x86_64-unknown-linux-gnu") \
					linker_setup="apt-get update && apt-get install -y gcc-x86-64-linux-gnu && export CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc"; \
					;; \
				"aarch64-unknown-linux-gnu") \
					linker_setup="apt-get update && apt-get install -y gcc-aarch64-linux-gnu && export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc"; \
					;; \
				"x86_64-pc-windows-gnu") \
					linker_setup="apt-get update && apt-get install -y gcc-mingw-w64-x86-64 && export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc"; \
					;; \
				"aarch64-pc-windows-gnullvm") \
					linker_setup="apt-get update && apt-get install -y gcc-mingw-w64 && export CC_aarch64_pc_windows_gnullvm=clang"; \
					;; \
				"x86_64-apple-darwin"|"aarch64-apple-darwin") \
					linker_setup=""; \
					;; \
				*) \
					linker_setup=""; \
					;; \
			esac; \
			break; \
		fi; \
	done; \
	if [ -z "$$target" ]; then \
		echo "❌ Unsupported platform: $(PLATFORM)"; \
		echo "   Available platforms: $(PLATFORMS)"; \
		exit 1; \
	fi; \
	echo "📦 Building for platform $(PLATFORM) (target: $$target) using Docker..."; \
	if [ -n "$$linker_setup" ]; then \
		$(rust_docker_run) sh -c "$$linker_setup && rustup target add $$target && CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc cargo build --release --target $$target"; \
	else \
		rustup target add $$target && cargo build --release --target $$target; \
	fi

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
cli-test:
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
	@echo "  all                    - Build and run all tests (default)"
	@echo "  build                  - Build using Docker (Linux binary on macOS, native platform on Linux)"
	@echo "  build_native           - Build natively using local Rust installation"
	@echo "  build_platform         - Build for specific platform using Docker (requires PLATFORM variable)"
	@echo "  test                   - Run all tests (unit and CLI) using Docker"
	@echo "  unit-test              - Run unit tests only using Docker"
	@echo "  cli-test               - Test CLI with sample data for both Android and PO formats using Docker"
	@echo "  clean                  - Clean build artifacts and temporary files using Docker"
	@echo "  help                   - Show this help message"
	@echo ""
	@echo "🎯 Platform-specific builds:"
	@for platform in $(PLATFORMS); do \
		echo "  make build_platform PLATFORM=$$platform"; \
	done
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
	@echo "  cd ruby && bundle exec rake build_rust[PLATFORM]"
	@echo "  cd ruby && bundle exec rake release_binary[PLATFORM]"
	@echo "  cd ruby && bundle exec rake release_all_platforms"
