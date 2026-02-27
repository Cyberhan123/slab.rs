# Slab.rs Backend Diagnostics

This directory contains diagnostic tools for troubleshooting the slab.rs backend, particularly for Whisper transcription issues.

## Diagnostic Script

### `diagnose-backend.sh`

A comprehensive diagnostic script that checks:

1. **System Dependencies** - cargo, ffmpeg, ffprobe, curl
2. **Environment Configuration** - All SLAB_* environment variables
3. **Whisper Library** - Verifies SLAB_WHISPER_LIB_DIR and library files
4. **Server Compilation** - Runs `cargo check -p slab-server`
5. **Database** - Checks if database file exists
6. **Server Startup** - Attempts to start the server and captures logs
7. **Health Check** - Verifies server health endpoint
8. **API Endpoints** - Tests all major API endpoints
9. **Whisper Backend** - Queries task API
10. **End-to-End Test** - Runs a test transcription if audio file available

### Usage

```bash
# Run diagnostics (output to current directory)
./diagnose-backend.sh

# Run diagnostics with custom output directory
./diagnose-backend.sh /path/to/output

# Run with environment variables set
SLAB_WHISPER_LIB_DIR=/path/to/whisper/lib ./diagnose-backend.sh
```

### Output

The script generates two files:

1. **`slab-diagnostic-TIMESTAMP.txt`** - Full diagnostic report with all checks
2. **`slab-summary-TIMESTAMP.txt`** - Quick summary with key findings
3. **`server-startup-TIMESTAMP.log`** - Server startup logs (if server was started)

### Interpreting Results

#### Success Case
```
Overall Status: ✓ PASSED
Success Rate: 100%

Critical Items:
1. Whisper Library: ✓ Configured
2. FFmpeg: ✓ Installed
3. Server: ✓ Running
4. Database: ✓ Exists
```

#### Common Issues

**Issue: SLAB_WHISPER_LIB_DIR not set**
```
✗ SLAB_WHISPER_LIB_DIR is NOT set - Whisper will NOT work
```
**Solution**: Set the environment variable pointing to your Whisper library directory:
```bash
export SLAB_WHISPER_LIB_DIR=/path/to/whisper/lib
```

**Issue: FFmpeg not installed**
```
✗ ffmpeg is NOT installed
```
**Solution**: Install FFmpeg:
```bash
# Ubuntu/Debian
sudo apt-get install ffmpeg

# macOS
brew install ffmpeg

# Arch Linux
sudo pacman -S ffmpeg
```

**Issue: Server fails to start**
```
✗ Server failed to start within 30 seconds
Check log file: server-startup-TIMESTAMP.log
```
**Solution**: Check the log file for specific errors. Common causes:
- Missing dependencies
- Database locked
- Port already in use
- Library loading failures

**Issue: Whisper library files not found**
```
✗ No Whisper library files found in /path/to/dir
```
**Solution**: Ensure the directory contains the Whisper shared library:
- Linux: `libwhisper.so`
- macOS: `libwhisper.dylib`
- Windows: `whisper.dll`

### Manual Testing

If the automated test doesn't find a suitable audio file, you can test manually:

```bash
# 1. Start the server (if not already running)
cargo run -p slab-server

# 2. In another terminal, submit a transcription request
curl -X POST http://localhost:3000/v1/audio/transcriptions \
  -H "Content-Type: application/json" \
  -d '{
    "path": "/path/to/your/audio.wav"
  }'

# Expected response:
# {"task_id":"<uuid>"}

# 3. Check task status
curl http://localhost:3000/v1/tasks/<uuid>

# 4. Get transcription result
curl http://localhost:3000/v1/tasks/<uuid>/result
```

### Troubleshooting Tips

1. **Check server logs** with `SLAB_LOG=debug` for verbose output
2. **Verify FFmpeg** can convert your audio format: `ffmpeg -i input.wav -f f32le -acodec pcm_f32le -ar 16000 -ac 1 -`
3. **Test Whisper library** loading independently if possible
4. **Check database** for stuck tasks: `sqlite3 slab.db "SELECT * FROM tasks WHERE status='running'"`
5. **Monitor system resources** during transcription (CPU, memory, GPU if applicable)

### Environment Variables Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `SLAB_WHISPER_LIB_DIR` | (none) | Directory containing Whisper shared library |
| `SLAB_LOG` | `info` | Log level (trace, debug, info, warn, error) |
| `SLAB_DATABASE_URL` | `sqlite://slab.db?mode=rwc` | Database connection string |
| `SLAB_BIND` | `0.0.0.0:3000` | Server bind address |
| `SLAB_QUEUE_CAPACITY` | `64` | Orchestrator queue size |
| `SLAB_BACKEND_CAPACITY` | `4` | Max concurrent requests per backend |

### Getting Help

If diagnostics fail and you can't resolve the issue:

1. Run with debug logging: `SLAB_LOG=debug ./diagnose-backend.sh`
2. Capture full server logs from the startup attempt
3. Check the GitHub issues for similar problems
4. Create a new issue with:
   - Diagnostic summary file
   - Server startup logs
   - OS and version
   - Rust version (`rustc --version`)
   - FFmpeg version (`ffmpeg -version`)
