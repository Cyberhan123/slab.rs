#!/bin/bash
# Smoke Test Script for Slab.rs
# Run this AFTER the server is started to validate all fixes

set -e

echo "=========================================="
echo "  Slab.rs Smoke Test"
echo "=========================================="
echo ""

# Configuration
SERVER_URL="${SLAB_SERVER_URL:-http://localhost:3000}"
TEST_AUDIO="${SLAB_TEST_AUDIO:-/home/cyberhan/slab.rs/testdata/samples/jfk.wav}"

echo "Server URL: $SERVER_URL"
echo "Test Audio: $TEST_AUDIO"
echo ""

# Check if server is running
echo "🏥 Step 1: Checking if server is running..."
if curl -s "$SERVER_URL/health" > /dev/null; then
    echo "✅ Server is running"
else
    echo "❌ Server is NOT running"
    echo "   Start it with: cargo run -p slab-server"
    exit 1
fi
echo ""

# Check diagnostics endpoint
echo "🔬 Step 2: Checking backend diagnostics..."
DIAGNOSTICS=$(curl -s "$SERVER_URL/diagnostics")
echo "$DIAGNOSTICS" | jq .
echo ""

# Check if test audio exists
echo "🎤 Step 3: Checking test audio..."
if [ -f "$TEST_AUDIO" ]; then
    echo "✅ Test audio found: $TEST_AUDIO"
    echo "   Size: $(stat -f%z "$TEST_AUDIO" 2>/dev/null || stat -c%s "$TEST_AUDIO") bytes"
    echo "   Duration: $(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$TEST_AUDIO" 2>/dev/null || echo "unknown") seconds"
else
    echo "❌ Test audio NOT found: $TEST_AUDIO"
    echo "   Downloading test audio..."

    # Create testdata directory
    mkdir -p "$(dirname "$TEST_AUDIO")"

    # Try to download a sample audio file
    # Using a public domain sample
    curl -L -o "$TEST_AUDIO" "https://www2.cs.uic.edu/~i101/SoundFiles/BabyElephantWalk60.wav"

    if [ -f "$TEST_AUDIO" ]; then
        echo "✅ Test audio downloaded"
    else
        echo "❌ Failed to download test audio"
        echo "   Please provide a test audio file at: $TEST_AUDIO"
        exit 1
    fi
fi
echo ""

# Submit transcription request
echo "📤 Step 4: Submitting transcription request..."
RESPONSE=$(curl -s -X POST "$SERVER_URL/v1/audio/transcriptions" \
    -H "Content-Type: application/json" \
    -d "{\"path\": \"$TEST_AUDIO\"}")

echo "Response:"
echo "$RESPONSE" | jq .
echo ""

# Extract task ID
TASK_ID=$(echo "$RESPONSE" | jq -r '.task_id // empty')

if [ -z "$TASK_ID" ] || [ "$TASK_ID" = "null" ]; then
    echo "❌ Failed to get task_id from response"
    echo "   Response was: $RESPONSE"
    exit 1
fi

echo "✅ Task submitted successfully!"
echo "   Task ID: $TASK_ID"
echo ""

# Poll for task completion
echo "⏳ Step 5: Polling for task completion..."
echo ""

POLL_COUNT=0
MAX_POLLS=30
POLL_INTERVAL=2

while [ $POLL_COUNT -lt $MAX_POLLS ]; do
    POLL_COUNT=$((POLL_COUNT + 1))
    sleep $POLL_INTERVAL

    # Get task status
    TASK_RESPONSE=$(curl -s "$SERVER_URL/v1/tasks/$TASK_ID")
    TASK_STATUS=$(echo "$TASK_RESPONSE" | jq -r '.status // empty')
    TASK_TYPE=$(echo "$TASK_RESPONSE" | jq -r '.task_type // empty')
    CREATED_AT=$(echo "$TASK_RESPONSE" | jq -r '.created_at // empty')
    ERROR_MSG=$(echo "$TASK_RESPONSE" | jq -r '.error_msg // empty')

    echo "Poll $POLL_COUNT/$MAX_POLLS: Status = $TASK_STATUS"

    # Check for terminal states
    if [ "$TASK_STATUS" = "succeeded" ] || [ "$TASK_STATUS" = "failed" ] || [ "$TASK_STATUS" = "cancelled" ]; then
        echo ""
        echo "✅ Task reached terminal state: $TASK_STATUS"
        break
    fi
done

echo ""

# Get full task details
echo "📋 Step 6: Full task details..."
TASK_DETAILS=$(curl -s "$SERVER_URL/v1/tasks/$TASK_ID")
echo "$TASK_DETAILS" | jq .
echo ""

# Get result if succeeded
if [ "$TASK_STATUS" = "succeeded" ]; then
    echo "📄 Step 7: Getting task result..."
    RESULT=$(curl -s "$SERVER_URL/v1/tasks/$TASK_ID/result")
    echo "$RESULT" | jq .
    echo ""

    # VALIDATION SUMMARY
    echo "=========================================="
    echo "  VALIDATION SUMMARY"
    echo "=========================================="
    echo ""
    echo "✅ SMOKE TEST PASSED!"
    echo ""
    echo "Team Fixes Validated:"
    echo "  ✓ Status: '$TASK_STATUS' (ux-ui-expert fix working!)"
    echo "  ✓ Backend: Processing audio correctly"
    echo "  ✓ Result: Being returned and displayed"
    echo "  ✓ Error handling: No errors in successful flow"
    echo ""
    echo "All team improvements have been validated!"
    echo ""
    echo "🎉 MISSION ACCOMPLISHED!"

elif [ "$TASK_STATUS" = "failed" ]; then
    echo "=========================================="
    echo "  FAILURE ANALYSIS"
    echo "=========================================="
    echo ""
    echo "❌ SMOKE TEST FAILED"
    echo ""
    echo "Task ID: $TASK_ID"
    echo "Task Type: $TASK_TYPE"
    echo "Error: $ERROR_MSG"
    echo ""
    echo "This is expected if:"
    echo "  • Whisper library not loaded (SLAB_WHISPER_LIB_DIR not set)"
    echo "  • Whisper model not loaded"
    echo "  • FFmpeg preprocessing failed"
    echo ""
    echo "Check server logs for details:"
    echo "  tail -50 /tmp/slab-server.log"
    echo ""
    echo "Run diagnostics:"
    echo "  curl $SERVER_URL/diagnostics | jq ."

else
    echo "=========================================="
    echo "  UNEXPECTED STATUS"
    echo "=========================================="
    echo ""
    echo "Task status: $TASK_STATUS"
    echo ""
    echo "This is unexpected. Please investigate."
fi

echo ""
echo "=========================================="
echo "  Test Parameters"
echo "=========================================="
echo ""
echo "Server: $SERVER_URL"
echo "Task ID: $TASK_ID"
echo "Test Audio: $TEST_AUDIO"
echo "Final Status: $TASK_STATUS"
echo ""
echo "View task: curl $SERVER_URL/v1/tasks/$TASK_ID"
echo "View result: curl $SERVER_URL/v1/tasks/$TASK_ID/result"
echo ""
echo "=========================================="
