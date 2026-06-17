import { renderHook } from '@testing-library/react';
import type { ChangeEvent } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const openMock = vi.hoisted(() => vi.fn<() => Promise<string | null>>());

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: openMock,
}));

import useFile from '../use-file';

function clearTauriInternals() {
  Reflect.deleteProperty(window, '__TAURI_INTERNALS__');
}

function setTauriInternals() {
  Object.defineProperty(window, '__TAURI_INTERNALS__', {
    configurable: true,
    value: {},
  });
}

function fileInputEvent(file?: File): ChangeEvent<HTMLInputElement> {
  const files = file
    ? ({
        0: file,
        item: (index: number) => (index === 0 ? file : null),
        length: 1,
      } as unknown as FileList)
    : null;

  return {
    target: {
      files,
    },
  } as ChangeEvent<HTMLInputElement>;
}

describe('useFile', () => {
  beforeEach(() => {
    clearTauriInternals();
    openMock.mockReset();
  });

  afterEach(() => {
    clearTauriInternals();
  });

  it('returns the first selected File in browser mode', async () => {
    const file = new File(['audio'], 'sample.wav', { type: 'audio/wav' });
    const { result } = renderHook(() => useFile());

    await expect(result.current.handleFile(fileInputEvent(file))).resolves.toEqual({
      file,
      name: 'sample.wav',
    });
    expect(openMock).not.toHaveBeenCalled();
  });

  it('returns null when browser mode receives no selected file', async () => {
    const { result } = renderHook(() => useFile());

    await expect(result.current.handleFile(fileInputEvent())).resolves.toBeNull();
  });

  it('opens the Tauri file dialog and derives the selected file name from the path', async () => {
    setTauriInternals();
    openMock.mockResolvedValueOnce('C:\\recordings\\voice.mp3');
    const { result } = renderHook(() => useFile());

    await expect(result.current.handleFile()).resolves.toEqual({
      file: 'C:\\recordings\\voice.mp3',
      name: 'voice.mp3',
    });
    expect(openMock).toHaveBeenCalledWith({
      filters: [
        { extensions: ['mp3', 'wav', 'flac', 'm4a', 'ogg'], name: 'Audio' },
        { extensions: ['mp4', 'mkv', 'webm'], name: 'Video' },
      ],
      multiple: false,
    });
  });

  it('returns null when the Tauri file dialog is cancelled', async () => {
    setTauriInternals();
    openMock.mockResolvedValueOnce(null);
    const { result } = renderHook(() => useFile());

    await expect(result.current.handleFile()).resolves.toBeNull();
  });
});
