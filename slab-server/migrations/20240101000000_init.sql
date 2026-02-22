-- Track every inbound API request for auditing and debugging.
-- The id column holds the trace_id UUID assigned by the tracing middleware.
CREATE TABLE IF NOT EXISTS request_log (
    id          TEXT    PRIMARY KEY,   -- trace_id (UUID v4)
    method      TEXT    NOT NULL,
    path        TEXT    NOT NULL,
    status      INTEGER,               -- HTTP status code; NULL until response written
    latency_ms  INTEGER,               -- round-trip latency ms; NULL until response written
    created_at  TEXT    NOT NULL       -- RFC 3339 timestamp
);
