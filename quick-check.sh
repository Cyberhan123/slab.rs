#!/bin/bash
################################################################################
# Quick Health Check Script for Slab.rs
#
# Fast checks for common issues. Use this for rapid troubleshooting.
# For comprehensive diagnostics, use diagnose-backend.sh instead.
################################################################################

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

ISSUES_FOUND=0

echo "=== Slab.rs Quick Health Check ==="
echo ""

# Function to check and report
check() {
    local description="$1"
    local command="$2"
    local fatal="${3:-false}"

    echo -n "Checking $description... "
    if eval "$command" > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
        return 0
    else
        echo -e "${RED}✗${NC}"
        if [ "$fatal" = "true" ]; then
            ISSUES_FOUND=$((ISSUES_FOUND + 1))
        fi
        return 1
    fi
}

warn() {
    local description="$1"
    local command="$2"

    echo -n "Checking $description... "
    if eval "$command" > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${YELLOW}!${NC} (warning)"
    fi
}

echo -e "${BLUE}System Dependencies${NC}"
check "cargo" "command -v cargo" true
check "ffmpeg" "command -v ffmpeg" true
check "ffprobe" "command -v ffprobe"
check "curl" "command -v curl" true
echo ""

echo -e "${BLUE}Environment${NC}"
warn "SLAB_WHISPER_LIB_DIR" "[ -n \"$SLAB_WHISPER_LIB_DIR\" ]"
warn "SLAB_DATABASE_URL" "[ -n \"$SLAB_DATABASE_URL\" ]"
warn "SLAB_LOG" "[ -n \"$SLAB_LOG\" ]"
echo ""

if [ -n "$SLAB_WHISPER_LIB_DIR" ]; then
    echo -e "${BLUE}Whisper Library${NC}"
    check "directory exists" "[ -d \"$SLAB_WHISPER_LIB_DIR\" ]" true
    check "contains library files" "find \"$SLAB_WHISPER_LIB_DIR\" -name \"*whisper*\" 2>/dev/null | grep -q ." true
    echo ""
fi

echo -e "${BLUE}Server Status${NC}"
if curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo -e "Server: ${GREEN}Running${NC} on port 3000"

    # Check API endpoints
    echo -n "API endpoints... "
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/v1/tasks 2>&1)
    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${YELLOW}! HTTP $HTTP_CODE${NC}"
    fi
else
    echo -e "Server: ${RED}Not Running${NC}"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi
echo ""

# Summary
if [ $ISSUES_FOUND -eq 0 ]; then
    echo -e "${GREEN}✓ No critical issues found${NC}"
    echo "Run './diagnose-backend.sh' for comprehensive diagnostics"
    exit 0
else
    echo -e "${RED}✗ $ISSUES_FOUND critical issue(s) found${NC}"
    echo ""
    echo "Quick fixes:"
    if ! command -v ffmpeg &> /dev/null; then
        echo "  • Install FFmpeg: sudo apt-get install ffmpeg (or brew install ffmpeg on macOS)"
    fi
    if [ -z "$SLAB_WHISPER_LIB_DIR" ]; then
        echo "  • Set SLAB_WHISPER_LIB_DIR to your Whisper library directory"
        echo "    Example: export SLAB_WHISPER_LIB_DIR=/usr/local/lib"
    fi
    if ! curl -s http://localhost:3000/health > /dev/null 2>&1; then
        echo "  • Start the server: cargo run -p slab-server"
    fi
    echo ""
    echo "Run './diagnose-backend.sh' for detailed diagnostics"
    exit 1
fi
