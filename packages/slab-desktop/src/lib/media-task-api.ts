import { SERVER_BASE_URL } from './config';

export type MediaTaskStatus =
  | 'pending'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'interrupted'
  | string;

export type MediaTaskProgress = {
  current?: number;
  total?: number;
  percent?: number;
  message?: string;
  stage?: string;
} | null;

export type ImageGenerationTask = {
  task_id: string;
  task_type: string;
  status: MediaTaskStatus;
  progress?: MediaTaskProgress;
  error_msg?: string | null;
  backend_id: string;
  model_id?: string | null;
  model_path: string;
  prompt: string;
  negative_prompt?: string | null;
  mode: string;
  width: number;
  height: number;
  requested_count: number;
  reference_image_url?: string | null;
  primary_image_url?: string | null;
  image_urls: string[];
  request_data: unknown;
  result_data?: unknown | null;
  created_at: string;
  updated_at: string;
};

export type VideoGenerationTask = {
  task_id: string;
  task_type: string;
  status: MediaTaskStatus;
  progress?: MediaTaskProgress;
  error_msg?: string | null;
  backend_id: string;
  model_id?: string | null;
  model_path: string;
  prompt: string;
  negative_prompt?: string | null;
  width: number;
  height: number;
  frames: number;
  fps: number;
  reference_image_url?: string | null;
  video_url?: string | null;
  request_data: unknown;
  result_data?: unknown | null;
  created_at: string;
  updated_at: string;
};

export type AudioTranscriptionTask = {
  task_id: string;
  task_type: string;
  status: MediaTaskStatus;
  progress?: MediaTaskProgress;
  error_msg?: string | null;
  backend_id: string;
  model_id?: string | null;
  source_path: string;
  language?: string | null;
  prompt?: string | null;
  detect_language?: boolean | null;
  vad_json?: unknown | null;
  decode_json?: unknown | null;
  transcript_text?: string | null;
  request_data: unknown;
  result_data?: unknown | null;
  created_at: string;
  updated_at: string;
};

function buildApiUrl(path: string): string {
  return new URL(path.replace(/^\//, ''), `${SERVER_BASE_URL}/`).toString();
}

export function resolveMediaUrl(path?: string | null): string | null {
  if (!path) {
    return null;
  }

  if (/^https?:\/\//i.test(path)) {
    return path;
  }

  return buildApiUrl(path);
}

async function readJson<T>(path: string): Promise<T> {
  const response = await fetch(buildApiUrl(path), {
    headers: {
      Accept: 'application/json',
    },
  });

  if (!response.ok) {
    let message = `${response.status} ${response.statusText}`;

    try {
      const errorBody = (await response.json()) as { message?: unknown; error?: unknown };
      const detail =
        typeof errorBody.message === 'string'
          ? errorBody.message
          : typeof errorBody.error === 'string'
            ? errorBody.error
            : null;
      if (detail) {
        message = detail;
      }
    } catch {
      // Ignore invalid JSON error bodies and keep the HTTP status text.
    }

    throw new Error(message);
  }

  return (await response.json()) as T;
}

export function listImageGenerations(): Promise<ImageGenerationTask[]> {
  return readJson<ImageGenerationTask[]>('/v1/images/generations');
}

export function getImageGeneration(taskId: string): Promise<ImageGenerationTask> {
  return readJson<ImageGenerationTask>(`/v1/images/generations/${taskId}`);
}

export function listVideoGenerations(): Promise<VideoGenerationTask[]> {
  return readJson<VideoGenerationTask[]>('/v1/video/generations');
}

export function getVideoGeneration(taskId: string): Promise<VideoGenerationTask> {
  return readJson<VideoGenerationTask>(`/v1/video/generations/${taskId}`);
}

export function listAudioTranscriptions(): Promise<AudioTranscriptionTask[]> {
  return readJson<AudioTranscriptionTask[]>('/v1/audio/transcriptions');
}

export function getAudioTranscription(taskId: string): Promise<AudioTranscriptionTask> {
  return readJson<AudioTranscriptionTask>(`/v1/audio/transcriptions/${taskId}`);
}
