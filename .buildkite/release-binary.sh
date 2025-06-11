#!/bin/bash -eu

BUILD_MODE="${1:-docker}"

case "$BUILD_MODE" in
  docker|native)
    echo "--- :hammer: Building Release Binary (mode: $BUILD_MODE)"
    ;;
  *)
    echo "Error: Invalid build mode '$BUILD_MODE'. Use 'docker' or 'native'"
    echo "Usage: $0 [docker|native]"
    exit 1
    ;;
esac

cd ruby
install_gems

bundle exec rake "release_binary[$BUILD_MODE]"
