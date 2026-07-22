import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  deriveProgress,
  getAudioTranscription,
  getImageGeneration,
  getVideoGeneration,
  listAudioTranscriptions,
  listImageGenerations,
  listVideoGenerations,
  resolveMediaUrl,
} from '../media-task-api';

// Hoisted mock handles so the factory can reference them without tripping the
// real openapi-fetch generic signatures. media-task-api consumes the named
// exports `apiClient.GET` and `ApiError.fromResponse` plus `SERVER_BASE_URL`.
const { apiGet, apiFromResponse } = vi.hoisted(() => ({
  apiGet: vi.fn<(...args: unknown[]) => Promise<unknown>>(),
  apiFromResponse: vi.fn<(response: unknown, error: unknown) => unknown>(),
}));

vi.mock('@slab/api', () => ({
  apiClient: { GET: apiGet },
  ApiError: { fromResponse: apiFromResponse },
}));

vi.mock('@slab/api/config', () => ({
  SERVER_BASE_URL: 'http://test.local',
}));

describe('deriveProgress', () => {
  it('returns queued state when no backend progress exists yet', () => {
    expect(deriveProgress(null, null, 1000)).toEqual({
      current: 0,
      etaMs: null,
      message: null,
      percent: null,
      stage: 'queued',
      stepLabel: null,
      total: null,
      updatedAt: 1000,
    });
  });

  it('projects percent, step labels, and ETA from current and previous samples', () => {
    const previous = deriveProgress(
      { current: 10, label: 'Sampling', message: null, step: 1, step_count: 2, total: 100 },
      null,
      1000,
    );

    expect(
      deriveProgress(
        { current: 30, label: 'Sampling', message: 'denoising', step: 1, step_count: 2, total: 100 },
        previous,
        3000,
      ),
    ).toEqual({
      current: 30,
      etaMs: 7000,
      message: 'denoising',
      percent: 30,
      stage: 'running',
      stepLabel: 'Sampling (1/2)',
      total: 100,
      updatedAt: 3000,
    });
  });

  it('marks nearly complete progress as finalizing and clamps impossible percentages', () => {
    expect(
      deriveProgress({ current: 120, label: 'Writing', total: 100 }, null, 1000),
    ).toMatchObject({
      percent: 100,
      stage: 'finalizing',
      stepLabel: 'Writing',
    });
  });
});

describe('resolveMediaUrl', () => {
  it('passes http(s) urls through unchanged', () => {
    expect(resolveMediaUrl('https://cdn.example/x.png')).toBe('https://cdn.example/x.png');
    expect(resolveMediaUrl('http://cdn.example/x.png')).toBe('http://cdn.example/x.png');
  });

  it('returns null for missing paths', () => {
    expect(resolveMediaUrl(null)).toBeNull();
    expect(resolveMediaUrl(undefined)).toBeNull();
    expect(resolveMediaUrl('')).toBeNull();
  });

  it('builds an absolute api url for relative paths', () => {
    expect(resolveMediaUrl('v1/files/x.png')).toBe('http://test.local/v1/files/x.png');
    expect(resolveMediaUrl('/v1/files/x.png')).toBe('http://test.local/v1/files/x.png');
  });
});

describe('requireApiData error handling (via getImageGeneration)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns data on a successful response and targets the detail path', async () => {
    apiGet.mockResolvedValue({ response: { ok: true } as Response, data: { id: 't1' } });
    await expect(getImageGeneration('t1')).resolves.toEqual({ id: 't1' });
    expect(apiGet).toHaveBeenCalledWith('/v1/images/generations/{id}', {
      params: { path: { id: 't1' } },
    });
  });

  it('throws an ApiError when the response is not ok', async () => {
    const response = { ok: false } as Response;
    apiGet.mockResolvedValue({ response, error: { detail: 'boom' } });
    apiFromResponse.mockReturnValue(new Error('api failed'));
    await expect(getImageGeneration('t1')).rejects.toThrow('api failed');
    expect(apiFromResponse).toHaveBeenCalledWith(response, { detail: 'boom' });
  });

  it('throws a descriptive error when data is missing on an ok response', async () => {
    apiGet.mockResolvedValue({ response: { ok: true } as Response });
    await expect(getImageGeneration('t1')).rejects.toThrow(
      "Image generation 't1' returned an empty response.",
    );
  });
});

describe('media list and get endpoints', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    apiGet.mockResolvedValue({ response: { ok: true } as Response, data: [] });
  });

  it('targets the image generation history and detail paths', async () => {
    await listImageGenerations();
    expect(apiGet).toHaveBeenCalledWith('/v1/images/generations');
    await getImageGeneration('t1');
    expect(apiGet).toHaveBeenCalledWith('/v1/images/generations/{id}', {
      params: { path: { id: 't1' } },
    });
  });

  it('targets the video generation paths', async () => {
    await listVideoGenerations();
    expect(apiGet).toHaveBeenCalledWith('/v1/video/generations');
    await getVideoGeneration('v1');
    expect(apiGet).toHaveBeenCalledWith('/v1/video/generations/{id}', {
      params: { path: { id: 'v1' } },
    });
  });

  it('targets the audio transcription paths', async () => {
    await listAudioTranscriptions();
    expect(apiGet).toHaveBeenCalledWith('/v1/audio/transcriptions');
    await getAudioTranscription('a1');
    expect(apiGet).toHaveBeenCalledWith('/v1/audio/transcriptions/{id}', {
      params: { path: { id: 'a1' } },
    });
  });

  it.each([
    [() => listImageGenerations(), 'Image generation history returned an empty response.'],
    [() => getImageGeneration('t1'), "Image generation 't1' returned an empty response."],
    [() => listVideoGenerations(), 'Video generation history returned an empty response.'],
    [() => getVideoGeneration('v1'), "Video generation 'v1' returned an empty response."],
    [() => listAudioTranscriptions(), 'Audio transcription history returned an empty response.'],
    [() => getAudioTranscription('a1'), "Audio transcription 'a1' returned an empty response."],
  ])('throws its dedicated empty-message when data is missing', async (call, message) => {
    apiGet.mockResolvedValue({ response: { ok: true } as Response });
    await expect(call()).rejects.toThrow(message);
  });
});
