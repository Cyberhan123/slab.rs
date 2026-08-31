import type { ReactNode } from 'react';
import type { ChangeEvent } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from 'vitest-browser-react';

import type { FileDialogPort } from '@slab/core';
import { SlabProvider } from '../../provider/slab-provider';
import { createTestSlabPorts, type TestSlabPortsOverrides } from '../../provider/test-ports';

import useFile from '../use-file';

const pickFileMock = vi.hoisted(() =>
  vi.fn<FileDialogPort['pickFile']>(),
);

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

const testFileDialog: FileDialogPort = {
  async pickFolder() {
    return null;
  },
  pickFile: pickFileMock,
  async pickFiles() {
    return [];
  },
};

function createWrapper(overrides: TestSlabPortsOverrides = {}) {
  return function wrapper({ children }: { children: ReactNode }) {
    return (
      <SlabProvider
        deps={{ ports: createTestSlabPorts({ fileDialog: testFileDialog, ...overrides }) }}
      >
        {children}
      </SlabProvider>
    );
  };
}

const desktopWrapper = createWrapper({
  platformInfo: { desktop: true, mobile: false, os: "unknown" },
});

describe('useFile', () => {
  beforeEach(() => {
    pickFileMock.mockReset();
  });

  it('returns the first selected File in browser mode', async () => {
    const file = new File(['audio'], 'sample.wav', { type: 'audio/wav' });
    const { result } = await renderHook(() => useFile(), { wrapper: createWrapper() });

    await expect(result.current.handleFile(fileInputEvent(file))).resolves.toEqual({
      file,
      name: 'sample.wav',
    });
    expect(pickFileMock).not.toHaveBeenCalled();
  });

  it('returns null when browser mode receives no selected file', async () => {
    const { result } = await renderHook(() => useFile(), { wrapper: createWrapper() });

    await expect(result.current.handleFile(fileInputEvent())).resolves.toBeNull();
  });

  it('opens the native file dialog and derives the selected file name from the path', async () => {
    pickFileMock.mockResolvedValueOnce({
      path: 'C:\\recordings\\voice.mp3',
      name: 'voice.mp3',
    });
    const { result } = await renderHook(() => useFile(), { wrapper: desktopWrapper });

    await expect(result.current.handleFile()).resolves.toEqual({
      file: 'C:\\recordings\\voice.mp3',
      name: 'voice.mp3',
    });
    expect(pickFileMock).toHaveBeenCalledWith({
      filters: [
        { extensions: ['mp3', 'wav', 'flac', 'm4a', 'ogg'], name: 'Audio' },
        { extensions: ['mp4', 'mkv', 'webm'], name: 'Video' },
      ],
      multiple: false,
    });
  });

  it('returns null when the native file dialog is cancelled', async () => {
    pickFileMock.mockResolvedValueOnce(null);
    const { result } = await renderHook(() => useFile(), { wrapper: desktopWrapper });

    await expect(result.current.handleFile()).resolves.toBeNull();
  });
});
