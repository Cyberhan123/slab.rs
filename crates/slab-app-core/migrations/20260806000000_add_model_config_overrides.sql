-- User overrides for pack-backed model load/inference parameters.
-- NULL means "use pack defaults"; a JSON object string replaces the pack
-- default value for the listed keys at load time. See ModelConfigStateRecord.
ALTER TABLE model_config_state ADD COLUMN load_overrides TEXT;
ALTER TABLE model_config_state ADD COLUMN inference_overrides TEXT;
