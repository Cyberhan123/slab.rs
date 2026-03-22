export const WHISPER_BACKEND_ID = 'ggml.whisper';
export const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
export const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

export type PreparingStage = 'prepare' | 'transcribe' | null;
