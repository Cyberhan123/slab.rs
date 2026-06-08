ALTER TABLE models ADD COLUMN materialized_artifacts TEXT NOT NULL DEFAULT '{}';

ALTER TABLE models ADD COLUMN selected_download_source TEXT;
