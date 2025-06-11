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
	@echo "   Convert Fluent to Android XML: ./target/release/fluent-tools android to-xml -i input.ftl -o output.xml"
	@echo "   Convert Android XML to Fluent: ./target/release/fluent-tools android to-fluent -i input.xml -o output.ftl"
	@echo "   Convert Fluent to PO: ./target/release/fluent-tools po to-po -i input.ftl -o output.po"
	@echo "   Convert PO to Fluent: ./target/release/fluent-tools po to-fluent -i input.po -o output.ftl"

# Run unit tests
unit-test:
	@echo "✅ Running unit tests with Docker..."
	$(rust_docker_run) cargo test -- --nocapture

# Test CLI with conversions for both Android and PO formats
cli-test: build
	@echo "🎯 Testing CLI with sample conversions using Docker..."
	
	# Android XML round trip test
	@echo "🤖 Testing Android XML conversion..."
	@echo "🔄 Step 1: Converting Fluent to Android XML..."
	$(rust_docker_run) ./target/release/fluent-tools android to-xml -i tests/data/sample_source.ftl -o test_android_output.xml
	
	@echo "🔄 Step 2: Converting Android XML back to Fluent..."
	$(rust_docker_run) ./target/release/fluent-tools android to-fluent -i test_android_output.xml -o test_android_roundtrip.ftl --original-fluent tests/data/sample_source.ftl
	
	@echo "🔍 Step 3: Verifying Android XML output files exist..."
	@if [ ! -f "test_android_output.xml" ] || [ ! -f "test_android_roundtrip.ftl" ]; then \
		echo "❌ Android XML output files not created"; \
		exit 1; \
	fi
	
	@echo "🔍 Step 4: Validating Android XML content..."
	@if grep -q '<string name="app-title"' test_android_output.xml; then \
		echo "✅ Android XML contains expected string entries"; \
	else \
		echo "❌ Android XML missing expected string entries"; \
		exit 1; \
	fi
	
	@if grep -q '%1$$s' test_android_output.xml; then \
		echo "✅ Android XML contains proper variable placeholders"; \
	else \
		echo "❌ Android XML missing variable placeholders"; \
		exit 1; \
	fi
	
	@if grep -q '<plurals name=' test_android_output.xml; then \
		echo "✅ Android XML contains plural forms"; \
	else \
		echo "❌ Android XML missing plural forms"; \
		exit 1; \
	fi
	
	@echo "🔍 Step 5: Validating Android XML roundtrip preservation..."
	@if grep -q 'app-title.*=.*My Application' test_android_roundtrip.ftl; then \
		echo "✅ Simple strings preserved in Android roundtrip"; \
	else \
		echo "❌ Simple strings not preserved in Android roundtrip"; \
		exit 1; \
	fi
	
	@if grep -q '{.*username.*}' test_android_roundtrip.ftl; then \
		echo "✅ Variables preserved in Android roundtrip"; \
	else \
		echo "❌ Variables not preserved in Android roundtrip"; \
		exit 1; \
	fi
	
	# PO format round trip test
	@echo "📝 Testing PO format conversion..."
	@echo "🔄 Step 6: Converting Fluent to PO..."
	$(rust_docker_run) ./target/release/fluent-tools po to-po -i tests/data/sample_source.ftl -o test_po_output.po
	
	@echo "🔄 Step 7: Converting PO back to Fluent..."
	$(rust_docker_run) ./target/release/fluent-tools po to-fluent -i test_po_output.po -o test_po_roundtrip.ftl
	
	@echo "🔍 Step 8: Verifying PO output files exist..."
	@if [ ! -f "test_po_output.po" ] || [ ! -f "test_po_roundtrip.ftl" ]; then \
		echo "❌ PO output files not created"; \
		exit 1; \
	fi
	
	@echo "🔍 Step 9: Validating PO file content..."
	@if grep -q 'msgid "My Application"' test_po_output.po; then \
		echo "✅ PO file contains expected message IDs"; \
	else \
		echo "❌ PO file missing expected message IDs"; \
		exit 1; \
	fi
	
	@if grep -q 'msgstr ""' test_po_output.po; then \
		echo "✅ PO file contains proper msgstr entries"; \
	else \
		echo "❌ PO file missing msgstr entries"; \
		exit 1; \
	fi
	
	@if grep -q 'Content-Type: text/plain' test_po_output.po; then \
		echo "✅ PO file contains proper header"; \
	else \
		echo "❌ PO file missing proper header"; \
		exit 1; \
	fi
	
	@echo "🔍 Step 10: Validating PO roundtrip preservation..."
	@if grep -q 'app-title.*=.*My Application' test_po_roundtrip.ftl; then \
		echo "✅ Simple strings preserved in PO roundtrip"; \
	else \
		echo "❌ Simple strings not preserved in PO roundtrip"; \
		exit 1; \
	fi
	
	@if grep -q 'welcome-user.*=.*{.*username.*}' test_po_roundtrip.ftl; then \
		echo "✅ Variables preserved in PO roundtrip"; \
	else \
		echo "❌ Variables not preserved in PO roundtrip"; \
		exit 1; \
	fi
	
	@if grep -A 10 'notification-count.*=' test_po_roundtrip.ftl | grep -q '{$$count ->' && \
	   grep -A 10 'notification-count.*=' test_po_roundtrip.ftl | grep -q '\[one\]' && \
	   grep -A 10 'notification-count.*=' test_po_roundtrip.ftl | grep -q '\*\[other\]'; then \
		echo "✅ Plural forms preserved in PO roundtrip"; \
	else \
		echo "❌ Plural forms not preserved in PO roundtrip"; \
		exit 1; \
	fi
	
	@echo "🔍 Step 11: Cross-format variable consistency check..."
	@android_var_count=$$(grep -o '{.*username.*}' test_android_roundtrip.ftl | wc -l); \
	 po_var_count=$$(grep -o '{.*username.*}' test_po_roundtrip.ftl | wc -l); \
	 if [ "$$android_var_count" -eq "$$po_var_count" ] && [ "$$android_var_count" -gt 0 ]; then \
		echo "✅ Variable counts consistent across formats"; \
	 else \
		echo "❌ Variable counts inconsistent: Android=$$android_var_count, PO=$$po_var_count"; \
		exit 1; \
	 fi
	
	@echo "✅ All CLI tests completed successfully!"

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
	@echo "💎 For Ruby gem development, use:"
	@echo "  cd ruby && bundle exec rake -T"
