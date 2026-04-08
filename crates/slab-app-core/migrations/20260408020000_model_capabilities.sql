-- Persist model placement/runtime capabilities as serialized JSON.

ALTER TABLE models ADD COLUMN capabilities TEXT NOT NULL DEFAULT '[]';
