#!/bin/bash -eu

PLATFORM="${1:-}"

if [ -z "$PLATFORM" ]; then
  echo "Error: Platform is required"
  echo "Usage: $0 PLATFORM"
  exit 1
fi

echo "--- :hammer: Building Release Binary for $PLATFORM"

cd ruby
install_gems
bundle exec rake "build_and_create_github_release[$PLATFORM]"
