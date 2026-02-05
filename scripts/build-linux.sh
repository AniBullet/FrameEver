#!/bin/bash
set -e

OUTPUT_ENCODING="UTF-8"
export LANG=en_US.UTF-8

echo "================================================================"
echo "  Cap aniBullet - Linux Build Script"
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
    echo "  Install: sudo apt install ffmpeg"
fi

if [ ! -d "node_modules" ]; then
    echo "  ERROR: node_modules not found"
    echo "  Please run: pnpm install"
    exit 1
fi

echo "  OK Prerequisites verified"
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
    echo "[1/1] Building Production version..."
    echo ""
    pnpm build:tauri --config src-tauri/tauri.prod.conf.json
else
    echo "[1/1] Building Development version..."
    echo ""
    pnpm build:tauri
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
    echo "  Duration: ${MINUTES}m ${SECONDS}s"
    echo ""
    
    OUTPUT_PATH="apps/desktop/src-tauri/target/release"
    BUNDLE_PATH="apps/desktop/src-tauri/target/release/bundle"
    
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
    
    if [ -d "$BUNDLE_PATH/deb" ]; then
        for file in "$BUNDLE_PATH/deb"/*.deb; do
            if [ -f "$file" ]; then
                SIZE=$(du -h "$file" | cut -f1)
                echo "  OK $(basename "$file") ($SIZE)"
            fi
        done
    fi
    
    if [ -d "$BUNDLE_PATH/appimage" ]; then
        for file in "$BUNDLE_PATH/appimage"/*.AppImage; do
            if [ -f "$file" ]; then
                SIZE=$(du -h "$file" | cut -f1)
                echo "  OK $(basename "$file") ($SIZE)"
            fi
        done
    fi
    
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
