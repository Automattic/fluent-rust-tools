#!/bin/bash

# CLI integration test script for fluent-tools
# This script tests both Android XML and PO format conversions

set -e  # Exit on any error

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Docker configuration
RUST_DOCKER_CONTAINER="public.ecr.aws/docker/library/rust:1.86.0"
DOCKER_CONTAINER_REPO_DIR="/app"
RUST_DOCKER_RUN="docker run --rm -v $(PWD):${DOCKER_CONTAINER_REPO_DIR} -w ${DOCKER_CONTAINER_REPO_DIR} -e CARGO_HOME=${DOCKER_CONTAINER_REPO_DIR}/.cargo ${RUST_DOCKER_CONTAINER}"

# Test files
SAMPLE_SOURCE="tests/data/sample_source.ftl"
ANDROID_OUTPUT="test_android_output.xml"
ANDROID_ROUNDTRIP="test_android_roundtrip.ftl"
PO_OUTPUT="test_po_output.po"
PO_ROUNDTRIP="test_po_roundtrip.ftl"

echo -e "${BLUE}🎯 Testing CLI with sample conversions using Docker...${NC}"

# Ensure the binary exists, build if necessary
if [ ! -f "./target/release/fluent-tools" ]; then
    echo -e "${YELLOW}⚠️  fluent-tools binary not found. Building now...${NC}"
    echo -e "${BLUE}📦 Building Rust CLI project with Docker...${NC}"
    ${RUST_DOCKER_RUN} cargo build --release
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Failed to build fluent-tools binary${NC}"
        exit 1
    fi
fi

# Clean up any existing test files
rm -f "${ANDROID_OUTPUT}" "${ANDROID_ROUNDTRIP}" "${PO_OUTPUT}" "${PO_ROUNDTRIP}"

#######################################
# Android XML Round Trip Test
#######################################

echo -e "${YELLOW}🤖 Testing Android XML conversion...${NC}"

echo -e "${BLUE}🔄 Step 1: Converting Fluent to Android XML...${NC}"
${RUST_DOCKER_RUN} ./target/release/fluent-tools android from-fluent -i "${SAMPLE_SOURCE}" -o "${ANDROID_OUTPUT}"

echo -e "${BLUE}🔄 Step 2: Converting Android XML back to Fluent...${NC}"
${RUST_DOCKER_RUN} ./target/release/fluent-tools android to-fluent -i "${ANDROID_OUTPUT}" -o "${ANDROID_ROUNDTRIP}" --original-fluent "${SAMPLE_SOURCE}"

echo -e "${BLUE}🔍 Step 3: Verifying Android XML output files exist...${NC}"
if [ ! -f "${ANDROID_OUTPUT}" ] || [ ! -f "${ANDROID_ROUNDTRIP}" ]; then
    echo -e "${RED}❌ Android XML output files not created${NC}"
    exit 1
fi

echo -e "${BLUE}🔍 Step 4: Validating Android XML content...${NC}"
if grep -q '<string name="app-title"' "${ANDROID_OUTPUT}"; then
    echo -e "${GREEN}✅ Android XML contains expected string entries${NC}"
else
    echo -e "${RED}❌ Android XML missing expected string entries${NC}"
    exit 1
fi

if grep -q '%1$s' "${ANDROID_OUTPUT}"; then
    echo -e "${GREEN}✅ Android XML contains proper variable placeholders${NC}"
else
    echo -e "${RED}❌ Android XML missing variable placeholders${NC}"
    exit 1
fi

if grep -q '<plurals name=' "${ANDROID_OUTPUT}"; then
    echo -e "${GREEN}✅ Android XML contains plural forms${NC}"
else
    echo -e "${RED}❌ Android XML missing plural forms${NC}"
    exit 1
fi

echo -e "${BLUE}🔍 Step 5: Validating Android XML roundtrip preservation...${NC}"
if grep -q 'app-title.*=.*My Application' "${ANDROID_ROUNDTRIP}"; then
    echo -e "${GREEN}✅ Simple strings preserved in Android roundtrip${NC}"
else
    echo -e "${RED}❌ Simple strings not preserved in Android roundtrip${NC}"
    exit 1
fi

if grep -q '{.*username.*}' "${ANDROID_ROUNDTRIP}"; then
    echo -e "${GREEN}✅ Variables preserved in Android roundtrip${NC}"
else
    echo -e "${RED}❌ Variables not preserved in Android roundtrip${NC}"
    exit 1
fi

#######################################
# PO Format Round Trip Test
#######################################

echo -e "${YELLOW}📝 Testing PO format conversion...${NC}"

echo -e "${BLUE}🔄 Step 6: Converting Fluent to PO...${NC}"
${RUST_DOCKER_RUN} ./target/release/fluent-tools po from-fluent -i "${SAMPLE_SOURCE}" -o "${PO_OUTPUT}"

echo -e "${BLUE}🔄 Step 7: Converting PO back to Fluent...${NC}"
${RUST_DOCKER_RUN} ./target/release/fluent-tools po to-fluent -i "${PO_OUTPUT}" -o "${PO_ROUNDTRIP}"

echo -e "${BLUE}🔍 Step 8: Verifying PO output files exist...${NC}"
if [ ! -f "${PO_OUTPUT}" ] || [ ! -f "${PO_ROUNDTRIP}" ]; then
    echo -e "${RED}❌ PO output files not created${NC}"
    exit 1
fi

echo -e "${BLUE}🔍 Step 9: Validating PO file content...${NC}"
if grep -q 'msgid "My Application"' "${PO_OUTPUT}"; then
    echo -e "${GREEN}✅ PO file contains expected message IDs${NC}"
else
    echo -e "${RED}❌ PO file missing expected message IDs${NC}"
    exit 1
fi

if grep -q 'msgstr ""' "${PO_OUTPUT}"; then
    echo -e "${GREEN}✅ PO file contains proper msgstr entries${NC}"
else
    echo -e "${RED}❌ PO file missing msgstr entries${NC}"
    exit 1
fi

if grep -q 'Content-Type: text/plain' "${PO_OUTPUT}"; then
    echo -e "${GREEN}✅ PO file contains proper header${NC}"
else
    echo -e "${RED}❌ PO file missing proper header${NC}"
    exit 1
fi

echo -e "${BLUE}🔍 Step 10: Validating PO roundtrip preservation...${NC}"
if grep -q 'app-title.*=.*My Application' "${PO_ROUNDTRIP}"; then
    echo -e "${GREEN}✅ Simple strings preserved in PO roundtrip${NC}"
else
    echo -e "${RED}❌ Simple strings not preserved in PO roundtrip${NC}"
    exit 1
fi

if grep -q 'welcome-user.*=.*{.*username.*}' "${PO_ROUNDTRIP}"; then
    echo -e "${GREEN}✅ Variables preserved in PO roundtrip${NC}"
else
    echo -e "${RED}❌ Variables not preserved in PO roundtrip${NC}"
    exit 1
fi

if grep -A 10 'notification-count.*=' "${PO_ROUNDTRIP}" | grep -q '{ $count ->' && \
   grep -A 10 'notification-count.*=' "${PO_ROUNDTRIP}" | grep -q '\[one\]' && \
   grep -A 10 'notification-count.*=' "${PO_ROUNDTRIP}" | grep -q '\*\[other\]'; then
    echo -e "${GREEN}✅ Plural forms preserved in PO roundtrip${NC}"
else
    echo -e "${RED}❌ Plural forms not preserved in PO roundtrip${NC}"
    exit 1
fi

#######################################
# Cross-format Consistency Check
#######################################

echo -e "${BLUE}🔍 Step 11: Cross-format variable consistency check...${NC}"
android_var_count=$(grep -o '{.*username.*}' "${ANDROID_ROUNDTRIP}" | wc -l)
po_var_count=$(grep -o '{.*username.*}' "${PO_ROUNDTRIP}" | wc -l)

if [ "${android_var_count}" -eq "${po_var_count}" ] && [ "${android_var_count}" -gt 0 ]; then
    echo -e "${GREEN}✅ Variable counts consistent across formats${NC}"
else
    echo -e "${RED}❌ Variable counts inconsistent: Android=${android_var_count}, PO=${po_var_count}${NC}"
    exit 1
fi

echo -e "${GREEN}✅ All CLI tests completed successfully!${NC}"
