# Frontend-Backend Integration Analysis

## Executive Summary

This document analyzes the integration between the React/Tauri frontend and Rust backend, identifying patterns, issues, and recommendations for improvement.

## Architecture Overview

### Frontend Stack
- **Framework**: React 19.1.0 with Vite 7.0.4
- **Desktop**: Tauri 2.0
- **State Management**: TanStack Query (React Query) 5.90.21
- **UI Components**: shadcn/ui with Radix UI primitives
- **Forms**: react-hook-form with zod validation
- **API Client**: openapi-fetch with openapi-react-query

### Backend Stack
- **Framework**: Axum HTTP server
- **Runtime**: Tokio async runtime
- **Database**: SQLite with SQLx
- **API**: OpenAPI/Swagger with utoipa

## Communication Patterns

### 1. HTTP/REST API (Primary)
**Location**: `/slab-app/src/lib/api/index.ts`

```typescript
const fetchClient = createFetchClient<paths>({
  baseUrl: getApiConfig().baseUrl,
});
const api = createClient(fetchClient);
```

**Status**: ✅ Well-implemented
- Uses OpenAPI-generated types
- Type-safe API calls
- Automatic request/response validation

**Issues Found**:
- ⚠️ No request interceptors for auth
- ⚠️ No global error handling
- ⚠️ No retry logic for failed requests
- ⚠️ No request/response logging

### 2. Tauri IPC Commands (Secondary)
**Location**: `/slab-app/src-tauri/src/lib.rs`

**Status**: 🔧 Partially implemented
- Basic commands added (`get_api_url`, `check_backend_status`)
- No direct backend integration via IPC
- Currently acts as HTTP proxy

**Issues Found**:
- ⚠️ Tauri commands don't directly call backend
- ⚠️ No file system operations via Tauri
- ⚠️ No native dialogs for file selection
- ⚠️ Missing commands for: file upload, model download status, native notifications

## Page-by-Page Analysis

### 1. Chat Page (`/pages/chat/`)

**Integration Status**: 🔴 Previously broken, ✅ Now fixed

**Previous Issues**:
- Hardcoded to `https://api.x.ant.design/api/big_model_glm-4.5-flash`
- No integration with Slab backend
- Demo-only functionality

**Fixes Applied**:
- ✅ Created `use-slab-chat.ts` hook for backend integration
- ✅ Implemented session management
- ✅ Added real chat UI (`slab-chat.tsx`)
- ✅ Mode switcher for demo vs. backend

**Remaining Issues**:
- ⚠️ Streaming responses not fully implemented
- ⚠️ No error recovery on backend failure
- ⚠️ No retry logic for failed messages

### 2. Audio Page (`/pages/audio/`)

**Integration Status**: ⚠️ Partially working

**Issues Found**:
```typescript
// use-transcribe.tsx:9
body: isTauri ? { path: value as string } : { path: "" }
```
- ⚠️ Web mode sends empty path (not implemented)
- ⚠️ Tauri mode expects file path string, but no file picker dialog
- ⚠️ No file validation before upload
- ⚠️ Error handling shows generic error messages

**Recommendations**:
1. Add Tauri file picker dialog
2. Validate file format (audio/*, video/*)
3. Show file size and duration
4. Add progress bar for long transcriptions
5. Implement real-time status updates

### 3. Image Page (`/pages/image/`)

**Integration Status**: ⚠️ Working but has issues

**Issues Found**:
```typescript
// index.tsx:67-106
const pollTaskStatus = async (id: string) => {
  // Manual polling with setTimeout
  setTimeout(() => pollTaskStatus(id), 2000);
}
```
- ⚠️ Manual polling instead of using React Query's polling
- ⚠️ No exponential backoff on errors
- ⚠️ Polling continues even if tab is hidden
- ⚠️ Image result handling is fragile (tries multiple formats)

**Recommendations**:
1. Use `refetchInterval` in React Query
2. Add WebSocket support for real-time updates
3. Implement proper image blob handling
4. Add gallery view for generated images
5. Support batch operations

### 4. Task Page (`/pages/task/`)

**Integration Status**: ✅ Well implemented

**Strengths**:
- ✅ Good use of React Query for data fetching
- ✅ Auto-refresh for running tasks
- ✅ Proper error handling with toasts
- ✅ Task status badges
- ✅ Cancel and restart functionality

**Issues Found**:
- ⚠️ No filtering or search
- ⚠️ No pagination for large task lists
- ⚠️ Task result display is basic
- ⚠️ No bulk operations
- ⚠️ Polling happens for ALL running tasks (could be optimized)

### 5. Hub Page (`/pages/hub/`)

**Integration Status**: ⚠️ Functional but incomplete

**Issues Found**:
```typescript
// index.tsx:100-116
const handleDownloadModel = async (values: DownloadFormValues) => {
  await downloadModelMutation.mutateAsync({
    params: { path: { repo_id: values.repo_id } },
    body: values,
  });
  toast.success('Model download initiated');
  // No progress tracking!
}
```
- ⚠️ No download progress tracking
- ⚠️ "Recent Actions" tab shows loading spinner forever
- ⚠️ No list of downloaded models
- ⚠️ No model version management

**Recommendations**:
1. Implement download progress via WebSocket or polling
2. Show list of available and downloaded models
3. Add model deletion
4. Implement model version switching
5. Add model validation (checksum, format)

### 6. Settings Page (`/pages/settings/`)

**Integration Status**: ✅ Good

**Strengths**:
- ✅ Clean separation of config and backends tabs
- ✅ Inline editing for config values
- ✅ Backend status checking
- ✅ Good error handling

**Issues Found**:
- ⚠️ No config validation before save
- ⚠️ No config type hints (text, number, boolean)
- ⚠️ Backend status requires manual refresh
- ⚠️ No "reset to defaults" option

## Error Handling Analysis

### Current State

**Toast Notifications**:
- ✅ Using `sonner` for toasts
- ✅ Consistent toast usage across pages
- ⚠️ Generic error messages
- ⚠️ No error codes displayed
- ⚠️ No "copy error" functionality

**Example from audio page**:
```typescript
// index.tsx:52-56
catch (err: any) {
  toast.error('创建转录任务失败', {
    description: err?.message || err?.error || '未知错误'
  });
}
```

### Issues Found

1. **Inconsistent Error Shapes**:
   - `err?.message`
   - `err?.error`
   - `err instanceof Error`
   - Backend returns different shapes

2. **No Error Boundaries**:
   - App crashes on unhandled errors
   - No fallback UI

3. **Silent Failures**:
   - Some mutations don't show errors
   - Polling stops without notification

## Recommendations

### High Priority

1. **Standardize Error Handling**:
```typescript
// Create error wrapper
export class ApiError extends Error {
  constructor(
    public code: string,
    public status: number,
    message: string
  ) {
    super(message);
  }
}

// Add error interceptor to fetch client
fetchClient.use({
  async onResponse({ response }) {
    if (!response.ok) {
      throw new ApiError(
        response.headers.get('x-error-code') || 'UNKNOWN',
        response.status,
        'Request failed'
      );
    }
  }
});
```

2. **Add Request Interceptors**:
```typescript
// Add auth token
fetchClient.use({
  async onRequest({ request }) {
    const token = localStorage.getItem('auth_token');
    if (token) {
      request.headers.set('Authorization', `Bearer ${token}`);
    }
    return request;
  }
});
```

3. **Implement Global Error Boundary**:
```typescript
// ErrorBoundary.tsx
export function ErrorBoundary({ children }: { children: ReactNode }) {
  return (
    <React.Suspense fallback={<ErrorFallback />}>
      <ErrorBoundaryFallback>{children}</ErrorBoundaryFallback>
    </React.Suspense>
  );
}
```

4. **Add Loading Skeletons**:
- Replace spinners with skeleton screens
- Show content placeholders during loading

### Medium Priority

5. **Add Request Retry Logic**:
```typescript
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (failureCount, error) => {
        if (error.status === 404) return false;
        return failureCount < 3;
      },
      retryDelay: (attemptIndex) => Math.min(1000 * 2 ** attemptIndex, 30000),
    },
  },
});
```

6. **Implement Request Cancellation**:
- Cancel pending requests on unmount
- Add abort controllers to long-running requests

7. **Add Request Logging**:
```typescript
fetchClient.use({
  async onRequest({ request }) {
    console.log(`[API] ${request.method} ${request.url}`);
    return request;
  },
});
```

### Low Priority

8. **Add Performance Monitoring**:
- Track API response times
- Monitor error rates
- Alert on degradation

9. **Add Offline Support**:
- Detect network status
- Queue requests when offline
- Show offline indicator

10. **Add Request Debugging**:
- DevTools panel for API calls
- Request/response inspector
- Replay failed requests

## Task Status Display Analysis

### Current Implementation

**Task Page** (`/pages/task/index.tsx`):
- ✅ Badge-based status display
- ✅ Color-coded statuses
- ✅ Status text in Chinese

**Status Mapping**:
```typescript
const getStatusBadge = (status: string) => {
  switch (status) {
    case 'pending': return <Badge variant="secondary">待处理</Badge>;
    case 'running': return <Badge variant="outline">运行中</Badge>;
    case 'completed': return <Badge variant="default">已完成</Badge>;
    case 'failed': return <Badge variant="destructive">失败</Badge>;
    case 'cancelled': return <Badge variant="outline">已取消</Badge>;
  }
};
```

### Issues Found

1. **Status Polling Inefficiency**:
   - Polls every 3 seconds for ALL running tasks
   - No deduplication of requests
   - Polls even when tab is hidden

2. **No Progress Information**:
   - Binary status (running/complete)
   - No percentage complete
   - No ETA

3. **Real-time Updates Missing**:
   - No WebSocket connection
   - No Server-Sent Events (SSE)
   - Manual polling only

### Recommendations

1. **Implement SSE for Real-time Updates**:
```typescript
// Backend already supports SSE for chat streaming
// Extend to task status updates
const eventSource = new EventSource('/v1/tasks/events');
eventSource.onmessage = (event) => {
  const update = JSON.parse(event.data);
  // Update task status
};
```

2. **Add Task Progress**:
```typescript
interface Task {
  id: string;
  status: string;
  progress?: number; // 0-100
  eta?: number; // seconds
  error?: string;
}
```

3. **Optimize Polling**:
```typescript
// Only poll selected task
useEffect(() => {
  if (!selectedTask || selectedTask.status !== 'running') return;

  const interval = setInterval(() => {
    fetchTaskDetail(selectedTask.id);
  }, 3000);

  return () => clearInterval(interval);
}, [selectedTask?.status, selectedTask?.id]);
```

## Conclusion

### Overall Assessment

**Integration Quality**: ⚠️ 6/10

**Strengths**:
- ✅ Type-safe API client with OpenAPI
- ✅ Consistent use of React Query
- ✅ Good UI/UX with shadcn/ui
- ✅ Proper loading states
- ✅ Toast notifications for feedback

**Critical Issues**:
- 🔴 Chat page was completely disconnected (FIXED)
- ⚠️ No global error handling
- ⚠️ Inconsistent error shapes
- ⚠️ No request retry logic
- ⚠️ Manual polling instead of real-time updates
- ⚠️ Tauri integration incomplete

### Priority Fixes

1. **High**: Standardize error handling and add error boundary
2. **High**: Implement real-time task updates via SSE
3. **Medium**: Add request retry and cancellation logic
4. **Medium**: Improve Tauri file picker integration
5. **Low**: Add performance monitoring and logging

### Testing Checklist

- [ ] Test all pages with backend offline
- [ ] Test error scenarios (400, 500, network errors)
- [ ] Test concurrent mutations
- [ ] Test request cancellation
- [ ] Test Tauri file picker
- [ ] Test task status updates
- [ ] Test long-running operations
- [ ] Test pagination and filtering
- [ ] Test form validation
- [ ] Test toast notifications
