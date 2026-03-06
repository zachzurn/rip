#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

# 1. Build native libraries for Android targets
echo "Building native libraries..."
cargo ndk -t arm64-v8a -t x86_64 \
    -o kotlin/src/main/jniLibs \
    build --release -p rip_android

# 2. Build the AAR
echo "Building AAR..."
cd kotlin
./gradlew assembleRelease

echo ""
echo "AAR ready at kotlin/build/outputs/aar/"
echo "To publish locally:  cd kotlin && ./gradlew publishToMavenLocal"
