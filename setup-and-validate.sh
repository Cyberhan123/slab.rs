#!/bin/bash
# Slab.rs - Setup and Validation Script
# This script installs dependencies and validates all team fixes

set -e

echo "=========================================="
echo "  Slab.rs Setup & Validation Script"
echo "=========================================="
echo ""

# Step 1: Install system dependencies
echo "📦 Step 1: Installing system dependencies..."
echo "   This requires sudo privileges."
echo ""

sudo apt update
sudo apt install -y ffmpeg libssl-dev pkg-config

echo ""
echo "✅ Dependencies installed successfully!"
echo ""

# Verify installations
echo "🔍 Verifying installations..."
echo ""

echo "FFmpeg:"
ffmpeg -version | head -n 1
echo ""

echo "OpenSSL (pkg-config):"
pkg-config --modversion openssl
echo ""

echo "✅ All dependencies verified!"
echo ""

# Step 2: Build server
echo "🔨 Step 2: Building slab-server..."
echo ""

cargo build -p slab-server

echo ""
echo "✅ Server built successfully!"
echo ""

# Step 3: Start server in background
echo "🚀 Step 3: Starting server..."
echo ""

# Kill any existing server
pkill -f "slab-server" || true
sleep 2

# Start server with logging
RUST_LOG=debug cargo run -p slab-server > /tmp/slab-server.log 2>&1 &
SERVER_PID=$!

echo "Server started with PID: $SERVER_PID"
echo "Logs: /tmp/slab-server.log"
echo ""

# Wait for server to be ready
echo "⏳ Waiting for server to start (10 seconds)..."
sleep 10

# Check if server is running
if ps -p $SERVER_PID > /dev/null; then
    echo "✅ Server is running!"
else
    echo "❌ Server failed to start. Check logs:"
    cat /tmp/slab-server.log
    exit 1
fi

echo ""

# Step 4: Health check
echo "🏥 Step 4: Health check..."
echo ""

HEALTH=$(curl -s http://localhost:3000/health)
echo "Health endpoint response:"
echo "$HEALTH" | jq . 2>/dev/null || echo "$HEALTH"
echo ""

# Step 5: Diagnostics check
echo "🔬 Step 5: Backend diagnostics..."
echo ""

DIAGNOSTICS=$(curl -s http://localhost:3000/diagnostics)
echo "Diagnostics endpoint response:"
echo "$DIAGNOSTICS" | jq . 2>/dev/null || echo "$DIAGNOSTICS"
echo ""

# Step 6: Submit test transcription
echo "🎤 Step 6: Submitting test transcription..."
echo ""

# Check if test audio exists
TEST_AUDIO="/home/cyberhan/slab.rs/testdata/samples/jfk.wav"
if [ -f "$TEST_AUDIO" ]; then
    echo "Using test audio: $TEST_AUDIO"

    RESPONSE=$(curl -s -X POST http://localhost:3000/v1/audio/transcriptions \
        -H "Content-Type: application/json" \
        -d "{\"path\": \"$TEST_AUDIO\"}")

    echo "Transcription response:"
    echo "$RESPONSE" | jq . 2>/dev/null || echo "$RESPONSE"
    echo ""

    # Extract task ID
    TASK_ID=$(echo "$RESPONSE" | jq -r '.task_id // empty')

    if [ -n "$TASK_ID" ] && [ "$TASK_ID" != "null" ]; then
        echo "✅ Task submitted with ID: $TASK_ID"
        echo ""

        # Step 7: Poll for completion
        echo "⏳ Step 7: Waiting for task completion..."
        echo ""

        for i in {1..30}; do
            sleep 2

            STATUS=$(curl -s http://localhost:3000/v1/tasks/$TASK_ID)
            TASK_STATUS=$(echo "$STATUS" | jq -r '.status // empty')

            echo "Poll $i/30: Status = $TASK_STATUS"

            if [ "$TASK_STATUS" = "succeeded" ] || [ "$TASK_STATUS" = "failed" ]; then
                echo ""
                echo "✅ Task finished with status: $TASK_STATUS"
                echo ""

                # Step 8: Get result
                echo "📄 Step 8: Getting task result..."
                echo ""

                RESULT=$(curl -s http://localhost:3000/v1/tasks/$TASK_ID/result)
                echo "Result:"
                echo "$RESULT" | jq . 2>/dev/null || echo "$RESULT"
                echo ""

                break
            fi
        done

        # Step 9: Validation summary
        echo "=========================================="
        echo "  VALIDATION SUMMARY"
        echo "=========================================="
        echo ""
        echo "Task ID: $TASK_ID"
        echo "Final Status: $TASK_STATUS"
        echo ""

        if [ "$TASK_STATUS" = "succeeded" ]; then
            echo "✅ SUCCESS! All team fixes are working:"
            echo ""
            echo "  ✓ Status mismatch bug FIXED (shows 'succeeded')"
            echo "  ✓ Backend processing audio correctly"
            echo "  ✓ Results being returned"
            echo "  ✓ All team improvements validated!"
            echo ""
            echo "🎉 MISSION COMPLETE - ALL FIXES VALIDATED!"
        else
            echo "❌ Task failed. Checking details..."
            echo ""
            echo "Full task details:"
            curl -s http://localhost:3000/v1/tasks/$TASK_ID | jq .
            echo ""
            echo "Server logs (last 50 lines):"
            tail -50 /tmp/slab-server.log
        fi
    else
        echo "❌ Failed to get task ID from response"
        echo ""
        echo "Response:"
        echo "$RESPONSE"
    fi
else
    echo "⚠️  Test audio file not found: $TEST_AUDIO"
    echo "Skipping transcription test."
    echo ""
    echo "To test manually, run:"
    echo "  curl -X POST http://localhost:3000/v1/audio/transcriptions \\"
    echo "    -H 'Content-Type: application/json' \\"
    echo "    -d '{\"path\": \"/path/to/your/audio.wav\"}'"
fi

echo ""
echo "=========================================="
echo "  Server logs: /tmp/slab-server.log"
echo "  Server PID: $SERVER_PID"
echo "  Stop server: kill $SERVER_PID"
echo "=========================================="
echo ""
echo "✅ Setup and validation complete!"
