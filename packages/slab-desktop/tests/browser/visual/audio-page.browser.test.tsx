import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import AudioPage from '@/pages/audio';
import type { SelectedFile } from '@/hooks/use-file';
import { renderDesktopScene } from '../test-utils';

const { mockUseAudio } = vi.hoisted(() => ({
  mockUseAudio: vi.fn<() => unknown>(),
}));

vi.mock('@/pages/audio/hooks/use-audio', () => ({
  useAudio: mockUseAudio,
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

vi.mock('@/hooks/use-persisted-header-select', () => ({
  usePersistedHeaderSelect: vi.fn<() => unknown>(() => ({
    value: 'model-1',
    setValue: vi.fn<() => void>(),
  })),
}));

const createVoidMock = () => vi.fn<(...args: unknown[]) => void>();
const createAsyncVoidMock = () =>
  vi.fn<(...args: unknown[]) => Promise<void>>().mockResolvedValue(undefined);

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
    handleFileChange: createAsyncVoidMock(),
    handleTauriFileSelect: createAsyncVoidMock(),
    handleTranscribe: createAsyncVoidMock(),
    hasBundledVad: true,
    isBusy: false,
    isTauri: true,
    isUsingBundledVad: false,
    navigate: createVoidMock(),
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
    setDecodeEntropyThold: createVoidMock(),
    setDecodeDurationMs: createVoidMock(),
    setDecodeLogprobThold: createVoidMock(),
    setDecodeMaxLen: createVoidMock(),
    setDecodeMaxTokens: createVoidMock(),
    setDecodeNoContext: createVoidMock(),
    setDecodeNoSpeechThold: createVoidMock(),
    setDecodeNoTimestamps: createVoidMock(),
    setDecodeOffsetMs: createVoidMock(),
    setDecodeSplitOnWord: createVoidMock(),
    setDecodeSuppressNst: createVoidMock(),
    setDecodeTdrzEnable: createVoidMock(),
    setDecodeTemperature: createVoidMock(),
    setDecodeTemperatureInc: createVoidMock(),
    setDecodeTokenTimestamps: createVoidMock(),
    setDecodeWordThold: createVoidMock(),
    setEnableVad: createVoidMock(),
    setSelectedVadModelId: createVoidMock(),
    setShowDecodeOptions: createVoidMock(),
    setVadMaxSpeechDurationS: createVoidMock(),
    setVadMinSilenceDurationMs: createVoidMock(),
    setVadMinSpeechDurationMs: createVoidMock(),
    setVadSamplesOverlap: createVoidMock(),
    setVadSpeechPadMs: createVoidMock(),
    setVadThreshold: createVoidMock(),
    showDecodeOptions: false,
    taskId: null,
    transcribe: {
      isPending: false,
      handleTranscribe: createVoidMock(),
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
