#!/bin/bash
set -e

export LANG=en_US.UTF-8

echo "================================================================"
echo "  Cap aniBullet - macOS Build Script"
echo "================================================================"
echo ""

echo "Checking prerequisites..."

if ! command -v node &> /dev/null; then
    echo "  ERROR: Node.js not found"
    echo "  Please install Node.js 20+"
    exit 1
fi

if ! command -v pnpm &> /dev/null; then
    echo "  ERROR: pnpm not found"
    echo "  Please install pnpm: npm install -g pnpm@10.5.2"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo "  ERROR: Rust/Cargo not found"
    echo "  Please install Rust from https://rustup.rs"
    exit 1
fi

if ! command -v ffmpeg &> /dev/null; then
    echo "  WARNING: FFmpeg not found"
    echo "  Install: brew install ffmpeg"
fi

if [ ! -d "node_modules" ]; then
    echo "  ERROR: node_modules not found"
    echo "  Please run: pnpm install"
    exit 1
fi

echo "  OK Prerequisites verified"
echo ""

ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    TARGET="aarch64-apple-darwin"
    ARCH_NAME="Apple Silicon"
else
    TARGET="x86_64-apple-darwin"
    ARCH_NAME="Intel"
fi

echo "Detected architecture: $ARCH_NAME ($TARGET)"
echo ""

echo "Select build type:"
echo "  [1] Development (Fast, for testing)"
echo "  [2] Production Release (Optimized, for distribution)"
echo ""
read -p "Enter option (1 or 2): " choice

BUILD_MODE=""

case $choice in
    1)
        BUILD_MODE="Development"
        echo ""
        echo "Selected: Development build"
        ;;
    2)
        BUILD_MODE="Production"
        echo ""
        echo "Selected: Production build"
        ;;
    *)
        echo ""
        echo "Invalid option, using Development"
        BUILD_MODE="Development"
        ;;
esac

echo ""
echo "================================================================"
echo "  Starting $BUILD_MODE Build"
echo "================================================================"
echo ""
echo "This may take 10-30 minutes..."
echo "Please ensure:"
echo "  1. Stable network connection"
echo "  2. Sufficient disk space (10GB+)"
echo "  3. Keep this terminal open"
echo ""

START_TIME=$(date +%s)

cd apps/desktop

if [ "$BUILD_MODE" = "Production" ]; then
    echo "[1/1] Building Production version for $ARCH_NAME..."
    echo ""
    pnpm build:tauri --target "$TARGET" --config src-tauri/tauri.prod.conf.json
else
    echo "[1/1] Building Development version for $ARCH_NAME..."
    echo ""
    pnpm build:tauri --target "$TARGET"
fi

BUILD_EXIT_CODE=$?
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
MINUTES=$((DURATION / 60))
SECONDS=$((DURATION % 60))

cd ../..

echo ""
echo "================================================================"

if [ $BUILD_EXIT_CODE -eq 0 ]; then
    echo "  Build Successful!"
    echo "================================================================"
    echo ""
    echo "Build info:"
    echo "  Mode: $BUILD_MODE"
    echo "  Architecture: $ARCH_NAME"
    echo "  Target: $TARGET"
    echo "  Duration: ${MINUTES}m ${SECONDS}s"
    echo ""
    
    OUTPUT_PATH="apps/desktop/src-tauri/target/$TARGET/release"
    BUNDLE_PATH="apps/desktop/src-tauri/target/$TARGET/release/bundle"
    
    if [ "$BUILD_MODE" = "Production" ]; then
        echo "Product Name: Cap aniBullet (Production)"
        echo "Identifier: so.cap.desktop.anibullet"
    else
        echo "Product Name: Cap aniBullet - Development"
        echo "Identifier: so.cap.desktop.anibullet.dev"
    fi
    
    echo ""
    echo "Output:"
    echo "  Executable: $OUTPUT_PATH"
    echo "  Packages: $BUNDLE_PATH"
    
    echo ""
    echo "Generated packages:"
    
    if [ -d "$BUNDLE_PATH/dmg" ]; then
        for file in "$BUNDLE_PATH/dmg"/*.dmg; do
            if [ -f "$file" ]; then
                SIZE=$(du -h "$file" | cut -f1)
                echo "  OK $(basename "$file") ($SIZE)"
            fi
        done
    fi
    
    if [ -d "$BUNDLE_PATH/macos" ]; then
        for file in "$BUNDLE_PATH/macos"/*.app; do
            if [ -d "$file" ]; then
                SIZE=$(du -sh "$file" | cut -f1)
                echo "  OK $(basename "$file") ($SIZE)"
            fi
        done
    fi
    
    echo ""
    echo "Note: This build is NOT code-signed or notarized."
    echo "To run, right-click the app and select 'Open' to bypass Gatekeeper."
    echo "Or run: xattr -cr /path/to/Cap\\ aniBullet.app"
    echo ""
    exit 0
else
    echo "  Build Failed"
    echo "================================================================"
    echo ""
    echo "Duration: ${MINUTES}m ${SECONDS}s"
    echo ""
    echo "If build failed: check dependencies and requirements. See README for details."
    echo ""
    exit 1
fi
