-- Classify memory usage citations by source artifact.

ALTER TABLE agent_memory_usage_events
    ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'unknown';

CREATE INDEX IF NOT EXISTS idx_agent_memory_usage_events_source_kind
    ON agent_memory_usage_events (source_kind, used_at);
