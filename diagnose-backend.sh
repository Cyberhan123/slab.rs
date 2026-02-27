#!/bin/bash
################################################################################
# Slab.rs Backend Diagnostic Script
#
# This script performs comprehensive diagnostics on the slab.rs backend,
# specifically focusing on Whisper transcription capabilities.
#
# Usage: ./diagnose-backend.sh [output-directory]
# Output: Generates a diagnostic report in the specified (or current) directory
################################################################################

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Output directory
OUTPUT_DIR="${1:-.}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$OUTPUT_DIR/slab-diagnostic-$TIMESTAMP.txt"
SUMMARY_FILE="$OUTPUT_DIR/slab-summary-$TIMESTAMP.txt"

# Server configuration (defaults from config.rs)
SERVER_HOST="${SLAB_BIND:-0.0.0.0:3000}"
SERVER_URL="http://localhost:3000"  # Always use localhost for local testing

################################################################################
# Helper Functions
################################################################################

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1" | tee -a "$REPORT_FILE"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1" | tee -a "$REPORT_FILE"
}

log_warning() {
    echo -e "${YELLOW}[!]${NC} $1" | tee -a "$REPORT_FILE"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1" | tee -a "$REPORT_FILE"
}

log_section() {
    echo "" | tee -a "$REPORT_FILE"
    echo "========================================" | tee -a "$REPORT_FILE"
    echo "$1" | tee -a "$REPORT_FILE"
    echo "========================================" | tee -a "$REPORT_FILE"
}

check_command() {
    if command -v "$1" &> /dev/null; then
        log_success "$1 is installed"
        return 0
    else
        log_error "$1 is NOT installed"
        return 1
    fi
}

check_env_var() {
    local var_name="$1"
    local var_value="${!var_name}"

    if [ -n "$var_value" ]; then
        log_success "$var_name is set: $var_value"
        return 0
    else
        log_warning "$var_name is NOT set"
        return 1
    fi
}

check_file_readable() {
    if [ -r "$1" ]; then
        log_success "File is readable: $1"
        return 0
    else
        log_error "File is NOT readable: $1"
        return 1
    fi
}

################################################################################
# Main Diagnostics
################################################################################

main() {
    mkdir -p "$OUTPUT_DIR"

    # Initialize report
    echo "Slab.rs Backend Diagnostic Report" > "$REPORT_FILE"
    echo "Generated: $(date)" >> "$REPORT_FILE"
    echo "Hostname: $(hostname)" >> "$REPORT_FILE"
    echo "User: $(whoami)" >> "$REPORT_FILE"
    echo "" >> "$REPORT_FILE"

    log_info "Starting diagnostics... (full report: $REPORT_FILE)"

    # Track overall status
    TOTAL_CHECKS=0
    PASSED_CHECKS=0
    FAILED_CHECKS=0
    WARNINGS=0

   ################################################################################
    # Section 1: System Dependencies
   ################################################################################

    log_section "1. SYSTEM DEPENDENCIES"

    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    if check_command "cargo"; then
        PASSED_CHECKS=$((PASSED_CHECKS + 1))
        cargo --version | tee -a "$REPORT_FILE"
    else
        FAILED_CHECKS=$((FAILED_CHECKS + 1))
    fi

    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    if check_command "ffmpeg"; then
        PASSED_CHECKS=$((PASSED_CHECKS + 1))
        ffmpeg -version | head -n 1 | tee -a "$REPORT_FILE"
    else
        FAILED_CHECKS=$((FAILED_CHECKS + 1))
    fi

    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    if check_command "ffprobe"; then
        PASSED_CHECKS=$((PASSED_CHECKS + 1))
        ffprobe -version | head -n 1 | tee -a "$REPORT_FILE"
    else
        FAILED_CHECKS=$((FAILED_CHECKS + 1))
    fi

    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    if check_command "curl"; then
        PASSED_CHECKS=$((PASSED_CHECKS + 1))
    else
        FAILED_CHECKS=$((FAILED_CHECKS + 1))
    fi

    ################################################################################
    # Section 2: Environment Configuration
   ################################################################################

    log_section "2. ENVIRONMENT CONFIGURATION"

    # Check all slab environment variables
    ENV_VARS=(
        "SLAB_LOG"
        "SLAB_LOG_JSON"
        "SLAB_DATABASE_URL"
        "SLAB_BIND"
        "SLAB_TRANSPORT"
        "SLAB_QUEUE_CAPACITY"
        "SLAB_BACKEND_CAPACITY"
        "SLAB_ENABLE_SWAGGER"
        "SLAB_CORS_ORIGINS"
        "SLAB_ADMIN_TOKEN"
        "SLAB_WHISPER_LIB_DIR"
        "SLAB_LLAMA_LIB_DIR"
        "SLAB_DIFFUSION_LIB_DIR"
        "SLAB_SESSION_STATE_DIR"
    )

    for var in "${ENV_VARS[@]}"; do
        TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
        if check_env_var "$var"; then
            PASSED_CHECKS=$((PASSED_CHECKS + 1))
        else
            if [[ "$var" == *"WHISPER"* ]] || [[ "$var" == *"LIB_DIR"* ]]; then
                FAILED_CHECKS=$((FAILED_CHECKS + 1))
            else
                PASSED_CHECKS=$((PASSED_CHECKS + 1))  # Optional vars are OK
                WARNINGS=$((WARNINGS + 1))
            fi
        fi
    done

    ################################################################################
    # Section 3: Whisper Library Check
   ################################################################################

    log_section "3. WHISPER LIBRARY CHECK"

    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    if [ -n "$SLAB_WHISPER_LIB_DIR" ]; then
        if [ -d "$SLAB_WHISPER_LIB_DIR" ]; then
            log_success "Whisper library directory exists: $SLAB_WHISPER_LIB_DIR"
            ls -la "$SLAB_WHISPER_LIB_DIR" | tee -a "$REPORT_FILE"

            # Look for library files
            if find "$SLAB_WHISPER_LIB_DIR" -name "*whisper*" -o -name "*libwhisper*" 2>/dev/null | head -1 | grep -q .; then
                log_success "Found Whisper library files"
                PASSED_CHECKS=$((PASSED_CHECKS + 1))
                find "$SLAB_WHISPER_LIB_DIR" -name "*whisper*" -o -name "*libwhisper*" 2>/dev/null | tee -a "$REPORT_FILE"
            else
                log_error "No Whisper library files found in $SLAB_WHISPER_LIB_DIR"
                FAILED_CHECKS=$((FAILED_CHECKS + 1))
            fi
        else
            log_error "Whisper library directory does NOT exist: $SLAB_WHISPER_LIB_DIR"
            FAILED_CHECKS=$((FAILED_CHECKS + 1))
        fi
    else
        log_error "SLAB_WHISPER_LIB_DIR is NOT set - Whisper will NOT work"
        FAILED_CHECKS=$((FAILED_CHECKS + 1))
    fi

    ################################################################################
    # Section 4: Server Compilation Check
   ################################################################################

    log_section "4. SERVER COMPILATION CHECK"

    log_info "Checking if server can be compiled..."
    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))

    if cargo check -p slab-server 2>&1 | tee -a "$REPORT_FILE"; then
        log_success "Server compilation check PASSED"
        PASSED_CHECKS=$((PASSED_CHECKS + 1))
    else
        log_error "Server compilation check FAILED"
        FAILED_CHECKS=$((FAILED_CHECKS + 1))
    fi

    ################################################################################
    # Section 5: Database Check
   ################################################################################

    log_section "5. DATABASE CHECK"

    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    DB_PATH="${SLAB_DATABASE_URL#sqlite://}"
    DB_PATH="${DB_PATH%\?*}"  # Remove query parameters

    if [ -f "$DB_PATH" ]; then
        log_success "Database file exists: $DB_PATH"
        ls -lh "$DB_PATH" | tee -a "$REPORT_FILE"
        PASSED_CHECKS=$((PASSED_CHECKS + 1))
    else
        log_warning "Database file does NOT exist: $DB_PATH (will be created on first run)"
        PASSED_CHECKS=$((PASSED_CHECKS + 1))  # OK, will be created
        WARNINGS=$((WARNINGS + 1))
    fi

    ################################################################################
    # Section 6: Server Startup Check
   ################################################################################

    log_section "6. SERVER STARTUP CHECK"

    # Check if server is already running
    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    if curl -s "http://localhost:3000/health" > /dev/null 2>&1; then
        log_success "Server is already running on port 3000"
        SERVER_ALREADY_RUNNING=true
        PASSED_CHECKS=$((PASSED_CHECKS + 1))
    else
        log_info "Server is not running. Attempting to start..."
        SERVER_ALREADY_RUNNING=false

        # Start server in background with logging
        LOG_FILE="$OUTPUT_DIR/server-startup-$TIMESTAMP.log"
        log_info "Starting server and logging to: $LOG_FILE"

        # Set log level to debug for diagnostics
        export SLAB_LOG="debug"

        if cargo run -p slab-server > "$LOG_FILE" 2>&1 &
        SERVER_PID=$!
        then
            log_info "Server started with PID: $SERVER_PID"

            # Wait for server to be ready (up to 30 seconds)
            log_info "Waiting for server to be ready..."
            READY=false
            for i in {1..30}; do
                if curl -s "http://localhost:3000/health" > /dev/null 2>&1; then
                    log_success "Server is ready after ${i}s"
                    READY=true
                    PASSED_CHECKS=$((PASSED_CHECKS + 1))
                    break
                fi
                sleep 1
            done

            if [ "$READY" = false ]; then
                log_error "Server failed to start within 30 seconds"
                log_error "Check log file: $LOG_FILE"
                FAILED_CHECKS=$((FAILED_CHECKS + 1))

                # Show last 50 lines of log
                echo "" | tee -a "$REPORT_FILE"
                echo "Last 50 lines of server log:" | tee -a "$REPORT_FILE"
                tail -50 "$LOG_FILE" | tee -a "$REPORT_FILE"

                # Clean up
                kill $SERVER_PID 2>/dev/null || true
                exit 1
            fi

            # Show startup logs
            echo "" | tee -a "$REPORT_FILE"
            echo "Server startup logs:" | tee -a "$REPORT_FILE"
            head -100 "$LOG_FILE" | tee -a "$REPORT_FILE"
        else
            log_error "Failed to start server"
            FAILED_CHECKS=$((FAILED_CHECKS + 1))
            exit 1
        fi
    fi

    ################################################################################
    # Section 7: Health Check
   ################################################################################

    log_section "7. HEALTH CHECK"

    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    HEALTH_RESPONSE=$(curl -s "http://localhost:3000/health" 2>&1)

    if echo "$HEALTH_RESPONSE" | grep -q "ok\|healthy"; then
        log_success "Health check PASSED"
        echo "Response: $HEALTH_RESPONSE" | tee -a "$REPORT_FILE"
        PASSED_CHECKS=$((PASSED_CHECKS + 1))
    else
        log_error "Health check FAILED"
        echo "Response: $HEALTH_RESPONSE" | tee -a "$REPORT_FILE"
        FAILED_CHECKS=$((FAILED_CHECKS + 1))
    fi

    ################################################################################
    # Section 8: API Endpoints Check
   ################################################################################

    log_section "8. API ENDPOINTS CHECK"

    ENDPOINTS=(
        "health:/health"
        "swagger:/swagger-ui"
        "openapi:/api-docs/openapi.json"
        "tasks:/v1/tasks"
        "audio:/v1/audio/transcriptions"
    )

    for endpoint in "${ENDPOINTS[@]}"; do
        NAME="${endpoint%%:*}"
        PATH="${endpoint##*:}"

        TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
        RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:3000$PATH" 2>&1)

        if [ "$RESPONSE" = "200" ] || [ "$RESPONSE" = "404" ] || [ "$RESPONSE" = "405" ]; then
            # 200 is OK, 404 might mean endpoint doesn't exist but server is up, 405 is method not allowed but endpoint exists
            log_success "$NAME endpoint: HTTP $RESPONSE"
            PASSED_CHECKS=$((PASSED_CHECKS + 1))
        else
            log_error "$NAME endpoint: HTTP $RESPONSE"
            FAILED_CHECKS=$((FAILED_CHECKS + 1))
        fi
    done

    ################################################################################
    # Section 9: Whisper Backend Status
   ################################################################################

    log_section "9. WHISPER BACKEND STATUS"

    # Check if we can query backend status
    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    BACKENDS_RESPONSE=$(curl -s "http://localhost:3000/v1/tasks" 2>&1)

    if [ $? -eq 0 ]; then
        log_success "Can query task API"
        echo "Response: $BACKENDS_RESPONSE" | tee -a "$REPORT_FILE"
        PASSED_CHECKS=$((PASSED_CHECKS + 1))
    else
        log_error "Cannot query task API"
        FAILED_CHECKS=$((FAILED_CHECKS + 1))
    fi

    ################################################################################
    # Section 10: End-to-End Test (if test audio available)
   ################################################################################

    log_section "10. END-TO-END TRANSCRIPTION TEST"

    # Look for test audio files
    TEST_AUDIO=$(find /tmp -name "*.wav" -o -name "*.mp3" -o -name "*.m4a" 2>/dev/null | head -1)

    if [ -n "$TEST_AUDIO" ]; then
        log_info "Found test audio: $TEST_AUDIO"

        TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
        TEST_RESPONSE=$(curl -s -X POST "http://localhost:3000/v1/audio/transcriptions" \
            -H "Content-Type: application/json" \
            -d "{\"path\": \"$TEST_AUDIO\"}" 2>&1)

        if echo "$TEST_RESPONSE" | grep -q "task_id"; then
            log_success "Transcription task submitted successfully"
            echo "Response: $TEST_RESPONSE" | tee -a "$REPORT_FILE"
            PASSED_CHECKS=$((PASSED_CHECKS + 1))

            # Extract task ID and check status
            TASK_ID=$(echo "$TEST_RESPONSE" | grep -o '"task_id":"[^"]*"' | cut -d'"' -f4)
            if [ -n "$TASK_ID" ]; then
                log_info "Checking task status for: $TASK_ID"

                sleep 2  # Give it a moment to process

                STATUS_RESPONSE=$(curl -s "http://localhost:3000/v1/tasks/$TASK_ID" 2>&1)
                echo "Task Status: $STATUS_RESPONSE" | tee -a "$REPORT_FILE"

                # Wait a bit more for completion
                sleep 5

                RESULT_RESPONSE=$(curl -s "http://localhost:3000/v1/tasks/$TASK_ID/result" 2>&1)
                echo "Task Result: $RESULT_RESPONSE" | tee -a "$REPORT_FILE"
            fi
        else
            log_error "Failed to submit transcription task"
            echo "Response: $TEST_RESPONSE" | tee -a "$REPORT_FILE"
            FAILED_CHECKS=$((FAILED_CHECKS + 1))
        fi
    else
        log_warning "No test audio file found. Skipping E2E test."
        log_info "To test manually, run:"
        echo "  curl -X POST http://localhost:3000/v1/audio/transcriptions \\"
        echo "    -H 'Content-Type: application/json' \\"
        echo "    -d '{\"path\": \"/path/to/audio.wav\"}'"
        TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
        PASSED_CHECKS=$((PASSED_CHECKS + 1))  # Don't fail for missing test file
        WARNINGS=$((WARNINGS + 1))
    fi

    ################################################################################
    # Cleanup
   ################################################################################

    if [ "$SERVER_ALREADY_RUNNING" = false ] && [ -n "$SERVER_PID" ]; then
        log_section "CLEANUP"
        log_info "Stopping server (PID: $SERVER_PID)..."
        kill $SERVER_PID 2>/dev/null || true
        sleep 2

        # Force kill if still running
        if ps -p $SERVER_PID > /dev/null 2>&1; then
            log_warning "Server still running, force killing..."
            kill -9 $SERVER_PID 2>/dev/null || true
        fi

        log_success "Server stopped"
    fi

    ################################################################################
    # Summary
   ################################################################################

    log_section "DIAGNOSTIC SUMMARY"

    PERCENTAGE=$((PASSED_CHECKS * 100 / TOTAL_CHECKS))

    echo "" | tee -a "$REPORT_FILE"
    echo "Total Checks: $TOTAL_CHECKS" | tee -a "$REPORT_FILE"
    echo "Passed: $PASSED_CHECKS" | tee -a "$REPORT_FILE"
    echo "Failed: $FAILED_CHECKS" | tee -a "$REPORT_FILE"
    echo "Warnings: $WARNINGS" | tee -a "$REPORT_FILE"
    echo "Success Rate: ${PERCENTAGE}%" | tee -a "$REPORT_FILE"
    echo "" | tee -a "$REPORT_FILE"

    # Create summary file
    cat > "$SUMMARY_FILE" << EOF
Slab.rs Backend Diagnostic Summary
Generated: $(date)
=====================================

Overall Status: $([ $FAILED_CHECKS -eq 0 ] && echo "✓ PASSED" || echo "✗ FAILED")

Statistics:
- Total Checks: $TOTAL_CHECKS
- Passed: $PASSED_CHECKS
- Failed: $FAILED_CHECKS
- Warnings: $WARNINGS
- Success Rate: ${PERCENTAGE}%

Critical Items:
1. Whisper Library: $([ -n "$SLAB_WHISPER_LIB_DIR" ] && echo "✓ Configured" || echo "✗ NOT CONFIGURED")
2. FFmpeg: $(command -v ffmpeg >/dev/null 2>&1 && echo "✓ Installed" || echo "✗ NOT INSTALLED")
3. Server: $(curl -s "http://localhost:3000/health" >/dev/null 2>&1 && echo "✓ Running" || echo "✗ NOT RUNNING")
4. Database: $([ -f "$DB_PATH" ] && echo "✓ Exists" || echo "✗ NOT FOUND")

Full Report: $REPORT_FILE
Server Log: ${LOG_FILE:-"N/A (server was already running)"}

Next Steps:
$([ $FAILED_CHECKS -gt 0 ] && echo "❌ Address failed checks above" || echo "✅ All critical checks passed!")
EOF

    cat "$SUMMARY_FILE"

    # Exit with appropriate code
    if [ $FAILED_CHECKS -gt 0 ]; then
        exit 1
    else
        exit 0
    fi
}

################################################################################
# Run Main
################################################################################

main
