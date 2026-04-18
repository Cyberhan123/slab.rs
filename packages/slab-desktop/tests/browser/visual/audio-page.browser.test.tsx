import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import AudioPage from '@/pages/audio';
import type { SelectedFile } from '@/hooks/use-file';
import { renderDesktopScene } from '../test-utils';

const { mockUseAudio } = vi.hoisted(() => ({
  mockUseAudio: vi.fn<
    () => Partial<ReturnType<typeof import('@/pages/audio/hooks/use-audio').useAudio>>
  >(),
}));

vi.mock('@/pages/audio/hooks/use-audio', () => ({
  useAudio: mockUseAudio,
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

vi.mock('@/hooks/use-persisted-header-select', () => ({
  usePersistedHeaderSelect: vi.fn<() => void>(() => ({ value: 'model-1', setValue: vi.fn<() => void>() })),
}));

function createAudioViewModel(overrides = {}) {
  return {
    bundledVadLabel: 'Bundled VAD',
    canStartTranscription: true,
    catalogModelsError: null,
    catalogModelsLoading: false,
    decodeEntropyThold: '',
    decodeDurationMs: '',
    decodeLogprobThold: '',
    decodeMaxLen: '',
    decodeMaxTokens: '',
    decodeNoContext: false,
    decodeNoSpeechThold: '',
    decodeNoTimestamps: false,
    decodeOffsetMs: '',
    decodeSplitOnWord: false,
    decodeSuppressNst: false,
    decodeTdrzEnable: false,
    decodeTemperature: '',
    decodeTemperatureInc: '',
    decodeTokenTimestamps: false,
    decodeWordThold: '',
    enableVad: false,
    file: null as SelectedFile | null,
    handleFileChange: vi.fn<() => void>(),
    handleTauriFileSelect: vi.fn<() => void>(),
    handleTranscribe: vi.fn<() => void>().mockResolvedValue(undefined),
    hasBundledVad: true,
    isBusy: false,
    isTauri: true,
    isUsingBundledVad: false,
    navigate: vi.fn<() => void>(),
    preparingStage: null as string | null,
    previewRows: [
      {
        label: 'Model',
        value: 'Not selected',
        accent: false,
        chip: true,
      },
      {
        label: 'Source',
        value: 'Awaiting upload',
        accent: false,
        chip: false,
      },
      {
        label: 'VAD Mode',
        value: 'Inactive',
        accent: false,
        chip: false,
      },
      {
        label: 'Decode',
        value: 'Default profile',
        accent: false,
        chip: false,
      },
    ],
    selectedVadModel: undefined,
    selectedVadModelId: '',
    setDecodeEntropyThold: vi.fn<() => void>(),
    setDecodeDurationMs: vi.fn<() => void>(),
    setDecodeLogprobThold: vi.fn<() => void>(),
    setDecodeMaxLen: vi.fn<() => void>(),
    setDecodeMaxTokens: vi.fn<() => void>(),
    setDecodeNoContext: vi.fn<() => void>(),
    setDecodeNoSpeechThold: vi.fn<() => void>(),
    setDecodeNoTimestamps: vi.fn<() => void>(),
    setDecodeOffsetMs: vi.fn<() => void>(),
    setDecodeSplitOnWord: vi.fn<() => void>(),
    setDecodeSuppressNst: vi.fn<() => void>(),
    setDecodeTdrzEnable: vi.fn<() => void>(),
    setDecodeTemperature: vi.fn<() => void>(),
    setDecodeTemperatureInc: vi.fn<() => void>(),
    setDecodeTokenTimestamps: vi.fn<() => void>(),
    setDecodeWordThold: vi.fn<() => void>(),
    setEnableVad: vi.fn<() => void>(),
    setSelectedVadModelId: vi.fn<() => void>(),
    setShowDecodeOptions: vi.fn<() => void>(),
    setVadMaxSpeechDurationS: vi.fn<() => void>(),
    setVadMinSilenceDurationMs: vi.fn<() => void>(),
    setVadMinSpeechDurationMs: vi.fn<() => void>(),
    setVadSamplesOverlap: vi.fn<() => void>(),
    setVadSpeechPadMs: vi.fn<() => void>(),
    setVadThreshold: vi.fn<() => void>(),
    showDecodeOptions: false,
    taskId: null,
    transcribe: {
      isPending: false,
      handleTranscribe: vi.fn<() => void>(),
    },
    vadMaxSpeechDurationS: '',
    vadMinSilenceDurationMs: '',
    vadMinSpeechDurationMs: '',
    vadSamplesOverlap: '',
    vadSpeechPadMs: '',
    vadThreshold: '',
    webFileInputRef: { current: null },
    whisperVadModels: [],
    ...overrides,
  };
}

describe('AudioPage browser visual regression', () => {
  beforeEach(() => {
    mockUseAudio.mockReset();
  });

  it('captures the audio workbench idle state', async () => {
    mockUseAudio.mockReturnValue(createAudioViewModel());

    await renderDesktopScene(<AudioPage />, { route: '/audio' });

    await expect.element(page.getByRole('heading', { name: /drag and drop/i })).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'audio-page-idle.png',
    );
  });

  it('captures the audio workbench with file selected', async () => {
    const mockFile = {
      name: 'test-audio.mp3',
      size: 1024 * 1024,
      type: 'audio/mpeg',
      file: new File([''], 'test-audio.mp3', { type: 'audio/mpeg' }),
    };

    mockUseAudio.mockReturnValue(
      createAudioViewModel({
        file: mockFile,
        previewRows: [
          {
            label: 'Model',
            value: 'Not selected',
            accent: false,
            chip: true,
          },
          {
            label: 'Source',
            value: 'test-audio.mp3',
            accent: true,
            chip: false,
          },
          {
            label: 'VAD Mode',
            value: 'Inactive',
            accent: false,
            chip: false,
          },
          {
            label: 'Decode',
            value: 'Default profile',
            accent: false,
            chip: false,
          },
        ],
      }),
    );

    await renderDesktopScene(<AudioPage />, { route: '/audio' });

    await expect.element(page.getByText('test-audio.mp3').first()).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'audio-page-with-file.png',
    );
  });

  it('captures the audio workbench busy state during transcription', async () => {
    mockUseAudio.mockReturnValue(
      createAudioViewModel({
        isBusy: true,
        preparingStage: 'transcribe',
        file: {
          name: 'transcribing.mp3',
          size: 1024 * 1024,
          type: 'audio/mpeg',
          file: new File([''], 'transcribing.mp3', { type: 'audio/mpeg' }),
        },
      }),
    );

    await renderDesktopScene(<AudioPage />, { route: '/audio' });

    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'audio-page-busy.png',
    );
  });
});
