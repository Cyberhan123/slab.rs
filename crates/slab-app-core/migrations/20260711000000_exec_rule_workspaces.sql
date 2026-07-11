-- Per-workspace exec-policy rule file mapping.
-- Records which `hash-<workspace>.rules` file (under the exec rules dir)
-- corresponds to which absolute workspace path, so the engine can lazy-load
-- the right per-workspace rules without scanning the whole directory.
CREATE TABLE IF NOT EXISTS exec_rule_workspaces (
    rules_filename  TEXT PRIMARY KEY,
    workspace_path  TEXT NOT NULL,
    created_at      TEXT NOT NULL
);
