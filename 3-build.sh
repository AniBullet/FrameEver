#!/bin/bash
set -e

export LANG=en_US.UTF-8

echo "================================================================"
echo "  Cap aniBullet - Build Application"
echo "================================================================"
echo ""

OS_TYPE=$(uname -s)

case "$OS_TYPE" in
    Darwin*)
        echo "Detected platform: macOS"
        echo ""
        echo "Starting macOS build script..."
        echo ""
        
        SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
        bash "$SCRIPT_DIR/scripts/build-macos.sh"
        ;;
    Linux*)
        echo "Detected platform: Linux"
        echo ""
        echo "Starting Linux build script..."
        echo ""
        
        SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
        bash "$SCRIPT_DIR/scripts/build-linux.sh"
        ;;
    *)
        echo "Unsupported platform: $OS_TYPE"
        echo "This script supports macOS and Linux only."
        echo "For Windows, run: .\3-build.ps1"
        exit 1
        ;;
esac
