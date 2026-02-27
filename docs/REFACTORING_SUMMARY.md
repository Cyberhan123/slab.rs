# Language-Specific Package Usage Refactoring

## Overview
Refactored slab-libfetch tools and frontend to use proper language-specific packages instead of shell commands and manual approaches.

## Rust Backend (slab-libfetch)

### Before: Shell Commands
```rust
// Used std::process::Command to call curl, tar, unzip
let status = std::process::Command::new("curl")
    .arg("-L")
    .arg("-o")
    .arg(dest)
    .arg(url)
    .status();
```

### After: Native Rust Crates
```rust
// Uses reqwest for HTTP downloads
let response = client.get(url).send().await?;
let bytes = response.bytes().await?;

// Uses flate2 + tar for tar.gz extraction
let decoder = GzDecoder::new(cursor);
let archive = Archive::new(decoder);
archive.unpack(dest_dir)?;

// Uses zip crate for zip extraction
let zip_archive = ZipArchive::new(cursor)?;
```

### Benefits
- ✅ Cross-platform compatibility (works on Windows, macOS, Linux)
- ✅ Better error handling with Result types
- ✅ No external dependencies on system utilities
- ✅ Async/await support for better performance
- ✅ Type safety throughout

## Frontend (slab-app)

### Settings Page Backend Management
Enhanced the settings page to provide a proper UI for backend installation:

#### Features
1. **Visual Status Indicators**
   - Green badge with CheckCircle2 icon for ready/running backends
   - Gray badge with XCircle icon for not configured
   - Yellow badge with AlertCircle icon for other statuses

2. **Download Progress**
   - Shows Loader2 spinner during download
   - Disables button while downloading
   - Provides success/error toast notifications

3. **Smart Button Display**
   - Only shows "Download" button for backends that aren't ready
   - Disables "Check Status" during download
   - Uses proper TypeScript types and react-query mutations

#### Code Quality
- Uses `openapi-fetch` for type-safe API calls
- Uses `@tanstack/react-query` for state management
- Uses `sonner` for toast notifications
- Uses `lucide-react` for consistent icons
- Proper TypeScript typing throughout

## Package Dependencies

### Rust (slab-libfetch)
```toml
[dependencies]
reqwest = { workspace = true }    # HTTP client
flate2 = { workspace = true }      # Gzip decompression
tar = { workspace = true }         # Tar archive extraction
zip = { workspace = true }         # Zip archive extraction
tokio = { workspace = true }       # Async runtime
anyhow = { workspace = true }      # Error handling
```

### Frontend (slab-app)
```json
{
  "dependencies": {
    "openapi-fetch": "latest",           // Type-safe API calls
    "@tanstack/react-query": "latest",   // State management
    "sonner": "latest",                  // Toast notifications
    "lucide-react": "latest",            // Icon library
    "@shadcn/ui": "latest"               // UI components
  }
}
```

## Startup Flow

### User Experience
1. **Launch Desktop Application** → Tauri app starts
2. **Settings Page** → User navigates to Backends tab
3. **Backend Status** → See which backends are ready/missing
4. **One-Click Install** → Click "Download" button for missing backends
5. **Automatic Setup** → Backend downloads and installs
6. **Launch Server** → slab-server starts with all backends ready

### Technical Flow
```
User clicks Download
  ↓
Frontend: downloadBackend(backendId)
  ↓
API: POST /admin/backends/download?backend_id=xxx
  ↓
Backend: Uses Rust crates to download & extract
  ↓
Frontend: Polls backend status until ready
  ↓
UI updates: Status changes to "Ready"
```

## Best Practices Applied

### ✅ DO
- Use language-specific packages (reqwest, tar, zip)
- Leverage async/await for I/O operations
- Provide proper error types and messages
- Use UI components from established libraries (shadcn/ui)
- Implement loading states and progress indicators
- Use TypeScript for type safety

### ❌ DON'T
- Use shell commands (curl, tar, unzip) from code
- Spawn child processes for package management
- Ignore error handling
- Block the UI during operations
- Use manual string parsing for structured data

## Testing Checklist

- [x] Rust version compiles without errors
- [x] Frontend TypeScript compiles without errors
- [x] Settings page displays backend status correctly
- [x] Download button shows loading state
- [x] Status badges use proper colors and icons
- [x] Error messages are user-friendly
- [x] All code comments and messages in English

## Future Enhancements

1. Add download progress percentage display
2. Support for custom backend library paths
3. Version selection for different model releases
4. Automatic dependency checking (CMake, make, etc.)
5. Retry logic for failed downloads
6. Concurrent download support for multiple backends
