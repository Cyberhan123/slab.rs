#!/bin/bash
# slab-libfetch - Shell script version
# Downloads GGML backend libraries for Slab

set -e

echo "=========================================="
echo "  Slab Library Fetcher"
echo "=========================================="
echo ""

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        PLATFORM="Linux"
        LIB_EXT="so"
        ;;
    Darwin)
        PLATFORM="macOS"
        LIB_EXT="dylib"
        ;;
    MINGW*|MSYS*|CYGWIN*)
        PLATFORM="Windows"
        LIB_EXT="dll"
        ;;
    *)
        echo "❌ Unknown OS: $OS"
        exit 1
        ;;
esac

echo "Detected platform: $PLATFORM ($ARCH)"
echo ""

# Create libraries directory
LIB_DIR="${SLAB_LIB_DIR:-./libraries}"
mkdir -p "$LIB_DIR"

echo "Library directory: $LIB_DIR"
echo ""

# Build Whisper library from source (v1.8.3)
# Note: No pre-built binary available for Linux - must build from source
echo "📦 Building Whisper library from source..."
WHISPER_DIR="$LIB_DIR/whisper"
mkdir -p "$WHISPER_DIR"

WHISPER_VERSION="v1.8.3"
WHISPER_REPO="https://github.com/ggml-org/whisper.cpp"

if [ "$PLATFORM" = "Linux" ]; then
    echo "  Cloning Whisper repository..."
    TEMP_DIR=$(mktemp -d)
    git clone --depth 1 --branch "$WHISPER_VERSION" "$WHISPER_REPO" "$TEMP_DIR/whisper.cpp"

    echo "  Building with CMake..."
    cd "$TEMP_DIR/whisper.cpp"
    cmake -B build
    cmake --build build -j --config Release

    echo "  Installing library..."
    cp "$TEMP_DIR/whisper.cpp/build/src/libwhisper.so"* "$WHISPER_DIR/"
    # Copy to versioned name
    cp "$TEMP_DIR/whisper.cpp/build/src/libwhisper.so"* "$WHISPER_DIR/libwhisper.so"

    rm -rf "$TEMP_DIR"
    cd - > /dev/null
elif [ "$PLATFORM" = "macOS" ]; then
    WHISPER_FILE="libwhisper.dylib"
    WHISPER_URL="https://github.com/ggml-org/whisper.cpp/releases/download/$WHISPER_VERSION/$WHISPER_FILE"
    echo "  Downloading: $WHISPER_URL"
    curl -L -o "$WHISPER_DIR/$WHISPER_FILE" "$WHISPER_URL"
else
    WHISPER_FILE="whisper.dll"
    WHISPER_URL="https://github.com/ggml-org/whisper.cpp/releases/download/$WHISPER_VERSION/$WHISPER_FILE"
    echo "  Downloading: $WHISPER_URL"
    curl -L -o "$WHISPER_DIR/$WHISPER_FILE" "$WHISPER_URL"
fi
echo "  ✅ Whisper library installed"
echo ""

# Download Llama library (b8170)
echo "📦 Fetching Llama library..."
LLAMA_DIR="$LIB_DIR/llama"
mkdir -p "$LLAMA_DIR"

LLAMA_VERSION="b8170"
LLAMA_BASE_URL="https://github.com/ggml-org/llama.cpp/releases/download"

if [ "$PLATFORM" = "Linux" ]; then
    # Download the Ubuntu x64 tar.gz package
    LLAMA_URL="$LLAMA_BASE_URL/$LLAMA_VERSION/llama-$LLAMA_VERSION-bin-ubuntu-x64.tar.gz"
    echo "  Downloading: $LLAMA_URL"
    TEMP_DIR=$(mktemp -d)
    curl -L -o "$TEMP_DIR/llama.tar.gz" "$LLAMA_URL"
    tar -xzf "$TEMP_DIR/llama.tar.gz" -C "$TEMP_DIR"
    # Copy all library files
    cp "$TEMP_DIR"/llama-$LLAMA_VERSION-bin-ubuntu-x64/libllama.so* "$LLAMA_DIR/"
    rm -rf "$TEMP_DIR"
elif [ "$PLATFORM" = "macOS" ]; then
    LLAMA_FILE="libllama.dylib"
    LLAMA_URL="$LLAMA_BASE_URL/$LLAMA_VERSION/$LLAMA_FILE"
    echo "  Downloading: $LLAMA_URL"
    curl -L -o "$LLAMA_DIR/$LLAMA_FILE" "$LLAMA_URL"
else
    LLAMA_FILE="llama.dll"
    LLAMA_URL="$LLAMA_BASE_URL/$LLAMA_VERSION/$LLAMA_FILE"
    echo "  Downloading: $LLAMA_URL"
    curl -L -o "$LLAMA_DIR/$LLAMA_FILE" "$LLAMA_URL"
fi
echo "  ✅ Llama library downloaded"
echo ""

# Download Stable Diffusion library (master-507-b314d80)
echo "📦 Fetching Stable Diffusion library..."
DIFFUSION_DIR="$LIB_DIR/diffusion"
mkdir -p "$DIFFUSION_DIR"

DIFFUSION_COMMIT="master-507-b314d80"
DIFFUSION_BASE_URL="https://github.com/leejet/stable-diffusion.cpp/releases/download"

if [ "$PLATFORM" = "Linux" ]; then
    # Download the Ubuntu 24.04 x86_64 zip package
    DIFFUSION_URL="$DIFFUSION_BASE_URL/$DIFFUSION_COMMIT/sd-master-b314d80-bin-Linux-Ubuntu-24.04-x86_64.zip"
    echo "  Downloading: $DIFFUSION_URL"
    TEMP_DIR=$(mktemp -d)
    curl -L -o "$TEMP_DIR/stable-diffusion.zip" "$DIFFUSION_URL"
    unzip -o "$TEMP_DIR/stable-diffusion.zip" -d "$TEMP_DIR"
    cp "$TEMP_DIR/libstable-diffusion.so" "$DIFFUSION_DIR/"
    rm -rf "$TEMP_DIR"
elif [ "$PLATFORM" = "macOS" ]; then
    DIFFUSION_FILE="libstable-diffusion.dylib"
    DIFFUSION_URL="$DIFFUSION_BASE_URL/$DIFFUSION_COMMIT/$DIFFUSION_FILE"
    echo "  Downloading: $DIFFUSION_URL"
    curl -L -o "$DIFFUSION_DIR/$DIFFUSION_FILE" "$DIFFUSION_URL"
else
    DIFFUSION_FILE="stable-diffusion.dll"
    DIFFUSION_URL="$DIFFUSION_BASE_URL/$DIFFUSION_COMMIT/$DIFFUSION_FILE"
    echo "  Downloading: $DIFFUSION_URL"
    curl -L -o "$DIFFUSION_DIR/$DIFFUSION_FILE" "$DIFFUSION_URL"
fi
echo "  ✅ Stable Diffusion library downloaded"
echo ""

echo "=========================================="
echo "  All libraries downloaded!"
echo "=========================================="
echo ""
echo "Set the following environment variables:"
echo "  export SLAB_LLAMA_LIB_DIR=$LLAMA_DIR"
echo "  export SLAB_WHISPER_LIB_DIR=$WHISPER_DIR"
echo "  export SLAB_DIFFUSION_LIB_DIR=$DIFFUSION_DIR"
echo ""
echo "Or add to your shell profile (~/.bashrc or ~/.zshrc):"
echo "  export SLAB_LLAMA_LIB_DIR=$(cd "$LLAMA_DIR" && pwd)"
echo "  export SLAB_WHISPER_LIB_DIR=$(cd "$WHISPER_DIR" && pwd)"
echo "  export SLAB_DIFFUSION_LIB_DIR=$(cd "$DIFFUSION_DIR" && pwd)"
