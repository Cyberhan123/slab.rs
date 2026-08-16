import type { components } from '@slab/api/v1';

import type { TranscribeOptions } from '../hooks/use-transcribe';

type AudioTranscriptionRequest = components['schemas']['AudioTranscriptionRequest'];

/**
 * Builds the audio transcription request body from desktop file path + options.
 * Pure data transform; throws on unsupported (web) or invalid path input so the
 * caller can surface the localized error. Extracted from useTranscribe for
 * direct unit testing.
 */
export function buildTranscriptionBody(
  value: File | string,
  options: TranscribeOptions | undefined,
  isTauri: boolean,
  t: (key: string) => string,
): AudioTranscriptionRequest {
  if (!isTauri) {
    throw new Error(t('pages.audio.error.webUploadNotImplemented'));
  }
  if (typeof value !== 'string' || !value.trim()) {
    throw new Error(t('pages.audio.error.invalidDesktopFilePath'));
  }

  const body: AudioTranscriptionRequest = { path: value };

  if (typeof options?.model_id === 'string' && options.model_id.trim()) {
    (body as AudioTranscriptionRequest & { model_id?: string }).model_id = options.model_id.trim();
  }
  if (typeof options?.language === 'string' && options.language.trim()) {
    body.language = options.language.trim();
  }
  if (typeof options?.prompt === 'string' && options.prompt.trim()) {
    body.prompt = options.prompt.trim();
  }
  if (options?.detect_language) {
    body.detect_language = true;
  }
  if (options?.vad) {
    body.vad = options.vad;
  }
  if (options?.decode) {
    body.decode = options.decode;
  }

  return body;
}
