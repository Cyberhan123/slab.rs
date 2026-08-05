import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from 'vitest-browser-react';

const { useIsTauriMock, mutateAsyncMock, useMutationMock } = vi.hoisted(() => ({
  useIsTauriMock: vi.fn<() => boolean>(),
  mutateAsyncMock: vi.fn<(payload: unknown) => Promise<unknown>>(),
  useMutationMock: vi.fn<() => unknown>(),
}));

vi.mock('@/hooks/use-tauri', () => ({ default: useIsTauriMock }));
vi.mock('@slab/api', () => ({ default: { useMutation: useMutationMock } }));
vi.mock('@slab/i18n', () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

import useTranscribe from '../use-transcribe';

function installMutation(overrides: Partial<{ isPending: boolean; isError: boolean; error: unknown }> = {}) {
  useMutationMock.mockReturnValue({
    isPending: false,
    isError: false,
    error: null,
    mutateAsync: mutateAsyncMock,
    ...overrides,
  });
}

describe('useTranscribe', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useIsTauriMock.mockReturnValue(true);
    mutateAsyncMock.mockResolvedValue({ operation_id: 'op-1' });
    installMutation();
  });

  it('subscribes to the transcription endpoint with the global error toast suppressed', async () => {
    await renderHook(() => useTranscribe());

    expect(useMutationMock).toHaveBeenCalledWith('post', '/v1/audio/transcriptions', {
      meta: { skipGlobalErrorToast: true },
    });
  });

  it('exposes the react-query mutation status', async () => {
    installMutation({ isPending: true, isError: true, error: new Error('boom') });
    const { result } = await renderHook(() => useTranscribe());

    expect(result.current.isPending).toBe(true);
    expect(result.current.isError).toBe(true);
    expect(result.current.error).toBeInstanceOf(Error);
  });

  it('builds the request body and submits it via mutateAsync', async () => {
    const { result, act } = await renderHook(() => useTranscribe());

    let response: unknown = null;
    await act(async () => {
      response = await result.current.handleTranscribe('/audio/a.mp3', { model_id: 'whisper' });
    });

    expect(mutateAsyncMock).toHaveBeenCalledWith({
      body: { path: '/audio/a.mp3', model_id: 'whisper' },
    });
    expect(response).toEqual({ operation_id: 'op-1' });
  });

  it('rejects submissions outside the desktop host', async () => {
    useIsTauriMock.mockReturnValue(false);
    const { result, act } = await renderHook(() => useTranscribe());

    let thrown: unknown = null;
    await act(async () => {
      try {
        await result.current.handleTranscribe('/audio/a.mp3');
      } catch (error) {
        thrown = error;
      }
    });

    expect(thrown).toBeInstanceOf(Error);
    expect((thrown as Error).message).toBe('pages.audio.error.webUploadNotImplemented');
    expect(mutateAsyncMock).not.toHaveBeenCalled();
  });

  it('rejects submissions with a non-string or blank path', async () => {
    const { result, act } = await renderHook(() => useTranscribe());

    let thrown: unknown = null;
    await act(async () => {
      try {
        await result.current.handleTranscribe('   ');
      } catch (error) {
        thrown = error;
      }
    });

    expect((thrown as Error).message).toBe('pages.audio.error.invalidDesktopFilePath');
    expect(mutateAsyncMock).not.toHaveBeenCalled();
  });
});
