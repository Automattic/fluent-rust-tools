#!/usr/bin/env bash

set -euo pipefail

echo "--- :xcode: Image toolchain"
xcodebuild -version
image_id="$1"
expected_xcode="${image_id#xcode-}"
xcodebuild -version | grep -q "^Xcode ${expected_xcode%%-*}$" \
  || { echo "ASSERTION FAILED: expected Xcode ${expected_xcode%%-*} on ${image_id}"; exit 1; }

echo "--- :rust: Installing Rust"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -v -y
# shellcheck disable=SC1091
source "$HOME/.cargo/env"

cd ruby
install_gems

for pair in x86_64-darwin:x86_64-apple-darwin:x86_64 arm64-darwin:aarch64-apple-darwin:arm64; do
  IFS=: read -r platform target arch <<< "$pair"
  binary="../target/$target/release/fluent-tools"
  rm -f "$binary"

  echo "--- :hammer: Building $platform"
  bundle exec rake "build_rust[$platform]"

  [ -x "$binary" ] || { echo "ASSERTION FAILED: $binary was not built"; exit 1; }
  archs=$(lipo -archs "$binary")
  echo "$binary: $archs"
  [ "$archs" = "$arch" ] || { echo "ASSERTION FAILED: expected $arch, got $archs"; exit 1; }
  if [ "$arch" = "$(uname -m)" ]; then "$binary" --help > /dev/null && echo "$binary runs"; fi
done
