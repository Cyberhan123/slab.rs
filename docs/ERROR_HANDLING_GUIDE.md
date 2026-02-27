# Error Handling Implementation Guide

## Overview

Implemented a consistent error handling system across the Rust backend and TypeScript frontend, using the standardized error response format:

```json
{
  "code": 4000,
  "data": null,
  "message": "Bad request: Invalid audio file path"
}
```

## Backend Implementation (Rust)

### Error Response Structure

**File**: `slab-server/src/error.rs`

```rust
#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    data: Option<serde_json::Value>,
    message: String,
}
```

### Error Codes

| Code | Name | Status Code | Description |
|------|------|-------------|-------------|
| 4000 | BAD_REQUEST | 400 | Invalid request parameters |
| 4004 | NOT_FOUND | 404 | Resource not found |
| 5003 | BACKEND_NOT_READY | 503 | Backend service unavailable |
| 5000 | RUNTIME_ERROR | 500 | AI runtime error |
| 5001 | DATABASE_ERROR | 500 | Database operation failed |
| 5002 | INTERNAL_ERROR | 500 | Internal server error |

### ServerError Enum

```rust
pub enum ServerError {
    Runtime(#[from] slab_core::RuntimeError),
    Database(#[from] sqlx::Error),
    NotFound(String),
    BadRequest(String),
    BackendNotReady(String),
    Internal(String),
}
```

### IntoResponse Implementation

```rust
impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, code, data, message) = match &self {
            // Match on error type and return appropriate response
        };

        let error_response = ErrorResponse { code, data, message };
        (status, Json(error_response)).into_response()
    }
}
```

## Frontend Implementation (TypeScript)

### Error Middleware

**File**: `slab-app/src/lib/api/errors.ts`

#### ApiError Class

```typescript
export class ApiError extends Error {
  code: number;
  data: unknown;

  constructor(code: number, message: string, data?: unknown) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.data = data;
  }

  isClientError(): boolean {
    return this.code >= 4000 && this.code < 5000;
  }

  isServerError(): boolean {
    return this.code >= 5000;
  }

  getUserMessage(): string {
    // Returns user-friendly message based on error code
  }
}
```

#### Error Middleware

```typescript
export const errorMiddleware: Middleware = {
  async onResponse({ response }) {
    if (response.ok) {
      return undefined;
    }

    const clonedResponse = response.clone();
    const errorData = await clonedResponse.json() as ApiErrorResponse;

    // Validate and throw ApiError
    if (typeof errorData.code === "number" &&
        typeof errorData.message === "string") {
      throw new ApiError(errorData.code, errorData.message, errorData.data);
    }

    throw new Error(`${response.url}: ${response.status} ${response.statusText}`);
  },

  async onError({ error }) {
    if (error instanceof Error) {
      return new ApiError(5002, error.message);
    }
    return error;
  },
};
```

### API Client Setup

**File**: `slab-app/src/lib/api/index.ts`

```typescript
import createFetchClient from "openapi-fetch";
import { errorMiddleware } from "./errors";

const fetchClient = createFetchClient<paths>({
  baseUrl: config.baseUrl,
});

// Register error middleware
fetchClient.use(errorMiddleware);

const api = createClient(fetchClient);
```

## Usage Examples

### Backend (Rust)

#### Returning Errors

```rust
use crate::error::ServerError;

pub async fn transcribe(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CompletionRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    // Validation
    if req.path.is_empty() {
        return Err(ServerError::BadRequest(
            "audio file path is empty".into()
        ));
    }

    if !path.exists() {
        return Err(ServerError::NotFound(format!(
            "Audio file does not exist: {}", req.path
        )));
    }

    // Backend check
    if !backend_ready {
        return Err(ServerError::BackendNotReady(
            "The Whisper backend is not ready".into()
        ));
    }

    // ... rest of handler
}
```

### Frontend (TypeScript)

#### Handling Errors in Components

```typescript
import api, { ApiError, getErrorMessage } from "@/lib/api";
import { toast } from "sonner";

const handleSubmit = async (data: MyData) => {
  try {
    await api.mutate("post", "/endpoint", { body: data });
    toast.success("Operation completed successfully");
  } catch (error) {
    // Automatically uses getUserMessage() for user-friendly text
    toast.error(getErrorMessage(error));
  }
};
```

#### Advanced Error Handling

```typescript
import { ApiError, isApiError, ErrorCodes } from "@/lib/api";

const handleApiCall = async () => {
  try {
    const result = await api.get("/endpoint");
    return result;
  } catch (error) {
    if (isApiError(error)) {
      // Check error type
      if (error.isClientError()) {
        // Handle client errors (4xxx)
        console.log("Client error:", error.code);
      } else if (error.isServerError()) {
        // Handle server errors (5xxx)
        console.log("Server error:", error.code);
      }

      // Check specific error codes
      if (error.code === ErrorCodes.BACKEND_NOT_READY) {
        // Show backend not ready UI
      }

      // Access additional error data
      if (error.data) {
        console.log("Error details:", error.data);
      }
    }

    throw error;
  }
};
```

## Benefits

### ✅ For Backend Developers
- **Consistent Format**: All errors follow the same structure
- **Easy to Extend**: Add new error codes as needed
- **Type Safety**: Rust's type system ensures correctness
- **Security**: Internal details logged, generic messages returned

### ✅ For Frontend Developers
- **Simple Error Handling**: One helper function `getErrorMessage(error)`
- **Type Safety**: TypeScript interfaces for error responses
- **User-Friendly Messages**: Automatic message generation from error codes
- **Easy to Debug**: Error codes and data always available

### ✅ For Users
- **Clear Messages**: User-friendly error messages
- **Consistent UX**: Errors look and behave the same everywhere
- **Actionable Feedback**: Know exactly what went wrong

## Testing

### Backend Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_response_format() {
        let error = ServerError::BadRequest("Invalid input".to_string());
        let response = error.into_response();

        // Verify response structure
        // Test error codes
        // Validate JSON output
    }
}
```

### Frontend Tests

```typescript
import { ApiError, getErrorMessage, ErrorCodes } from "./errors";

describe("ApiError", () => {
  it("should create ApiError with correct properties", () => {
    const error = new ApiError(4000, "Bad request", null);
    expect(error.code).toBe(4000);
    expect(error.message).toBe("Bad request");
    expect(error.isClientError()).toBe(true);
  });

  it("should return user-friendly message", () => {
    const error = new ApiError(4004, "Not found", null);
    expect(error.getUserMessage()).toBe(
      "The requested resource was not found."
    );
  });
});
```

## Migration Guide

### For Existing Backend Code

**Before**:
```rust
Json(json!({ "error": "message" }))
```

**After**:
```rust
// Automatically handled by IntoResponse implementation
Err(ServerError::BadRequest("message".into()))
```

### For Existing Frontend Code

**Before**:
```typescript
try {
  await api.post("/endpoint");
} catch (error: any) {
  toast.error(error.message || "Unknown error");
}
```

**After**:
```typescript
try {
  await api.post("/endpoint");
} catch (error) {
  toast.error(getErrorMessage(error));
}
```

## Best Practices

### ✅ DO
- Use specific error types (BadRequest, NotFound, etc.)
- Include helpful error messages
- Log internal errors with full details
- Return user-friendly messages to clients
- Use `getErrorMessage()` for UI display
- Check `isApiError()` before accessing error properties

### ❌ DON'T
- Expose internal implementation details in error messages
- Use generic errors when specific ones exist
- Ignore error codes
- Create custom error handling in each component
- Expose file paths, SQL, or other sensitive data

## Troubleshooting

### Common Issues

1. **"Cannot read property 'code' of undefined"**
   - Ensure error middleware is registered
   - Check that backend returns consistent error format

2. **Error messages not user-friendly**
   - Verify `getUserMessage()` is being called
   - Check that error codes match defined constants

3. **Type errors with error data**
   - Use type guards: `if (isApiError(error))`
   - Cast data to appropriate type: `error.data as MyType`

## Future Enhancements

- [ ] Add error logging/monitoring integration
- [ ] Support for error translation (i18n)
- [ ] Error rate limiting and circuit breakers
- [ ] Detailed error data for specific error types
- [ ] Error analytics and reporting
