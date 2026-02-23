-- Track every inbound API request for auditing and debugging.
-- The id column holds the trace_id UUID assigned by the tracing middleware.
CREATE TABLE IF NOT EXISTS request_log (
    id          TEXT    PRIMARY KEY,   -- trace_id (UUID v4, stored as 36-char string)
    method      TEXT    NOT NULL,
    path        TEXT    NOT NULL,
    status      INTEGER,               -- HTTP status code (i64); NULL until response written
    latency_ms  INTEGER,               -- round-trip latency ms; NULL until response written
    created_at  TEXT    NOT NULL       -- RFC 3339 timestamp
);

-- Indexes to improve query performance on common filter columns.
-- Especially useful once administrative endpoints query the audit log.
CREATE INDEX IF NOT EXISTS idx_request_log_created_at ON request_log(created_at);
CREATE INDEX IF NOT EXISTS idx_request_log_method     ON request_log(method);
CREATE INDEX IF NOT EXISTS idx_request_log_path       ON request_log(path);
