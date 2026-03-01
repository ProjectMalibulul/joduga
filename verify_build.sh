#!/bin/bash
# Joduga Build Verification Script
# This script checks if all dependencies are installed and the build succeeds

set -e

echo "🎵 Joduga Build Verification"
echo "============================"
echo ""

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

check_command() {
    if command -v $1 &> /dev/null; then
        echo -e "${GREEN}✓${NC} $1 found: $(command -v $1)"
        return 0
    else
        echo -e "${RED}✗${NC} $1 not found"
        return 1
    fi
}

echo "Checking dependencies..."
echo ""

# Check for Rust
if ! check_command cargo; then
    echo -e "${RED}ERROR: Rust is not installed${NC}"
    echo "Install from: https://rustup.rs/"
    exit 1
fi

# Check for C++ compiler
if ! check_command g++; then
    echo -e "${YELLOW}WARNING: g++ not found, trying clang++${NC}"
    if ! check_command clang++; then
        echo -e "${RED}ERROR: No C++ compiler found${NC}"
        echo "Install with: sudo apt install build-essential"
        exit 1
    fi
fi

# Check for CMake
if ! check_command cmake; then
    echo -e "${RED}ERROR: CMake is not installed${NC}"
    echo "Install with: sudo apt install cmake"
    exit 1
fi

echo ""
echo "All dependencies found!"
echo ""

# Check project structure
echo "Checking project structure..."
echo ""

if [ ! -f "CMakeLists.txt" ]; then
    echo -e "${RED}✗ CMakeLists.txt not found${NC}"
    echo "Are you in the joduga/ root directory?"
    exit 1
fi
echo -e "${GREEN}✓${NC} CMakeLists.txt found"

if [ ! -f "rust/Cargo.toml" ]; then
    echo -e "${RED}✗ rust/Cargo.toml not found${NC}"
    exit 1
fi
echo -e "${GREEN}✓${NC} rust/Cargo.toml found"

if [ ! -d "cpp/src" ]; then
    echo -e "${RED}✗ cpp/src directory not found${NC}"
    exit 1
fi
echo -e "${GREEN}✓${NC} cpp/src directory found"

echo ""
echo "Project structure looks good!"
echo ""

# Try to build
echo "Attempting build..."
echo ""

cd rust

if cargo build --release 2>&1 | tee /tmp/joduga_build.log; then
    echo ""
    echo -e "${GREEN}✅ Build successful!${NC}"
    echo ""
    echo "Run the test with:"
    echo "  cargo run --release"
    exit 0
else
    echo ""
    echo -e "${RED}❌ Build failed${NC}"
    echo ""
    echo "Check the error messages above."
    echo "Full build log saved to: /tmp/joduga_build.log"
    echo ""
    echo "Common fixes:"
    echo "  • Missing ALSA: sudo apt install libasound2-dev"
    echo "  • CMake version: Ensure cmake >= 3.20"
    exit 1
fi
