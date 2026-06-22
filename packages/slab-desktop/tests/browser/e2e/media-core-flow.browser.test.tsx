import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import AudioPage from '@/pages/audio';
import ImagePage from '@/pages/image';
import VideoPage from '@/pages/video';
import { renderDesktopScene } from '../test-utils';

const {
  mediaHarnessState,
  mockImageCancel,
  mockImageHistoryDetail,
  mockImageSubmit,
  mockAudioCancel,
  mockAudioHistoryDetail,
  mockAudioTranscribe,
  mockVideoCancel,
  mockVideoHistoryDetail,
  mockVideoSubmit,
} = vi.hoisted(() => ({
  mediaHarnessState: {
    audioRunning: false,
    imageGenerating: false,
    videoGenerating: false,
  },
  mockAudioCancel: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockAudioHistoryDetail: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockAudioTranscribe: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockImageCancel: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockImageHistoryDetail: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockImageSubmit: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockVideoCancel: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockVideoHistoryDetail: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockVideoSubmit: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
}));

vi.mock('@/pages/image/hooks/use-image-generation', async () => {
  const React = await import('react');

  return {
    useImageGeneration: vi.fn<() => unknown>(() => {
      const [prompt, setPrompt] = React.useState('');

      return {
        activeDimensionPreset: '1:1',
        advancedOpen: false,
        cfgScale: 7,
        clipSkip: 0,
        eta: 0,
        generationProgress: mediaHarnessState.imageGenerating
          ? {
              etaMs: 12_000,
              percent: 42,
              stage: 'running',
              stepLabel: 'sampling',
            }
          : null,
        guidance: 3.5,
        handleCancel: mockImageCancel,
        handleDimensionPreset: vi.fn<() => void>(),
        handleDownload: vi.fn<() => void>(),
        handleInitImageChange: vi.fn<() => void>(),
        handleSubmit: mockImageSubmit,
        heightStr: '512',
        history: [
          {
            backend_id: 'ggml.diffusion',
            created_at: '2026-06-01T10:00:00Z',
            error_msg: null,
            height: 512,
            image_urls: [],
            mode: 'txt2img',
            model_id: 'sdxl',
            model_path: 'C:/models/sdxl.gguf',
            primary_image_url: null,
            prompt: 'previous image prompt',
            status: 'succeeded',
            task_id: 'image-history-1',
            width: 512,
          },
        ],
        historyDialogOpen: false,
        historyError: null,
        historyLoading: false,
        images: [],
        initImageDataUri: null,
        initImageInputRef: { current: null },
        isBusy: false,
        isGenerating: mediaHarnessState.imageGenerating,
        isPreparingModel: false,
        isResolvingModelState: false,
        mode: 'txt2img',
        negativePrompt: '',
        numImages: 1,
        openHistoryDetail: mockImageHistoryDetail,
        parsedHeight: 512,
        parsedWidth: 512,
        prompt,
        sampleMethod: 'euler_a',
        scheduler: 'normal',
        seed: -1,
        selectedHistoryTask: null,
        selectedModelId: 'model-1',
        setAdvancedOpen: vi.fn<() => void>(),
        setCfgScale: vi.fn<() => void>(),
        setClipSkip: vi.fn<() => void>(),
        setEta: vi.fn<() => void>(),
        setGuidance: vi.fn<() => void>(),
        setHeightStr: vi.fn<() => void>(),
        setHistoryDialogOpen: vi.fn<() => void>(),
        setInitImageDataUri: vi.fn<() => void>(),
        setMode: vi.fn<() => void>(),
        setNegativePrompt: vi.fn<() => void>(),
        setNumImages: vi.fn<() => void>(),
        setPrompt,
        setSampleMethod: vi.fn<() => void>(),
        setScheduler: vi.fn<() => void>(),
        setSelectedHistoryTask: vi.fn<() => void>(),
        setSeed: vi.fn<() => void>(),
        setSteps: vi.fn<() => void>(),
        setStrength: vi.fn<() => void>(),
        setWidthStr: vi.fn<() => void>(),
        setZoomedImage: vi.fn<() => void>(),
        steps: 20,
        strength: 0.75,
        widthStr: '512',
        zoomedImage: null,
      };
    }),
  };
});

vi.mock('@/pages/video/hooks/use-video-generation', async () => {
  const React = await import('react');

  return {
    useVideoGeneration: vi.fn<() => unknown>(() => {
      const [prompt, setPrompt] = React.useState('');

      return {
        advancedOpen: false,
        cfgScale: 7,
        footerHint: 'Clip duration: 2.0s',
        fps: 8,
        frames: 16,
        generationProgress: mediaHarnessState.videoGenerating
          ? {
              etaMs: 18_000,
              percent: 64,
              stage: 'running',
              stepLabel: 'frames',
            }
          : null,
        guidance: 3.5,
        handleCancel: mockVideoCancel,
        handleDownload: vi.fn<() => void>(),
        handleInitImageChange: vi.fn<() => void>(),
        handleInitImageDrop: vi.fn<() => void>(),
        handleSubmit: mockVideoSubmit,
        heightStr: '512',
        heightValue: 512,
        hasSelectedModel: true,
        history: [
          {
            backend_id: 'ggml.diffusion',
            created_at: '2026-06-01T10:00:00Z',
            error_msg: null,
            fps: 8,
            frames: 16,
            height: 512,
            model_id: 'svd',
            model_path: 'C:/models/svd.gguf',
            prompt: 'previous video prompt',
            status: 'succeeded',
            task_id: 'video-history-1',
            video_url: null,
            width: 512,
          },
        ],
        historyDialogOpen: false,
        historyError: null,
        historyLoading: false,
        immersivePreview: false,
        initImageDataUri: null,
        initImageInputRef: { current: null },
        isGenerating: mediaHarnessState.videoGenerating,
        negativePrompt: '',
        openHistoryDetail: mockVideoHistoryDetail,
        prompt,
        sampleMethod: 'euler_a',
        scheduler: 'normal',
        seed: -1,
        selectedHistoryTask: null,
        setAdvancedOpen: vi.fn<() => void>(),
        setCfgScale: vi.fn<() => void>(),
        setFps: vi.fn<() => void>(),
        setFrames: vi.fn<() => void>(),
        setGuidance: vi.fn<() => void>(),
        setHeightStr: vi.fn<() => void>(),
        setHistoryDialogOpen: vi.fn<() => void>(),
        setImmersivePreview: vi.fn<() => void>(),
        setInitImageDataUri: vi.fn<() => void>(),
        setNegativePrompt: vi.fn<() => void>(),
        setPrompt,
        setSampleMethod: vi.fn<() => void>(),
        setScheduler: vi.fn<() => void>(),
        setSelectedHistoryTask: vi.fn<() => void>(),
        setSeed: vi.fn<() => void>(),
        setSteps: vi.fn<() => void>(),
        setStrength: vi.fn<() => void>(),
        setWidthStr: vi.fn<() => void>(),
        stageDescription: mediaHarnessState.videoGenerating
          ? 'Rendering frames'
          : 'Enter a prompt to generate video',
        stageStatus: mediaHarnessState.videoGenerating ? 'Running' : 'Awaiting prompt',
        stageTitle: mediaHarnessState.videoGenerating ? 'Generating' : 'Ready',
        steps: 20,
        strength: 0.75,
        videoPath: null,
        widthStr: '512',
        widthValue: 512,
      };
    }),
  };
});

vi.mock('@/pages/audio/hooks/use-audio', () => ({
  useAudio: vi.fn<() => unknown>(() => ({
    bundledVadLabel: 'Bundled VAD',
    canStartTranscription: true,
    catalogModelsError: null,
    catalogModelsLoading: false,
    decodeDurationMs: '',
    decodeEntropyThold: '',
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
    file: {
      file: new File(['audio'], 'meeting.mp3', { type: 'audio/mpeg' }),
      name: 'meeting.mp3',
      size: 512,
      type: 'audio/mpeg',
    },
    generationProgress: mediaHarnessState.audioRunning
      ? {
          etaMs: 9_000,
          percent: 35,
          stage: 'running',
          stepLabel: 'decode',
        }
      : null,
    handleCancelTranscription: mockAudioCancel,
    handleFileChange: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    handleTauriFileSelect: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    handleTranscribe: mockAudioTranscribe,
    hasBundledVad: true,
    history: [
      {
        backend_id: 'ggml.whisper',
        created_at: '2026-06-01T10:00:00Z',
        error_msg: null,
        language: 'en',
        model_id: 'whisper-large',
        prompt: null,
        source_path: 'C:/audio/meeting.mp3',
        status: 'succeeded',
        task_id: 'audio-history-1',
        transcript_text: 'hello world transcript',
      },
    ],
    historyDialogOpen: false,
    historyError: null,
    historyLoading: false,
    isBusy: mediaHarnessState.audioRunning,
    isCancellingTranscription: false,
    isTauri: true,
    isTranscriptionRunning: mediaHarnessState.audioRunning,
    isUsingBundledVad: false,
    openHistoryDetail: mockAudioHistoryDetail,
    preparingStage: null,
    previewRows: [
      {
        accent: false,
        chip: true,
        label: 'Model',
        value: 'Whisper Large',
      },
      {
        accent: true,
        chip: false,
        label: 'Source',
        value: 'meeting.mp3',
      },
    ],
    selectedHistoryTask: null,
    selectedVadModel: undefined,
    selectedVadModelId: '',
    setDecodeDurationMs: vi.fn<() => void>(),
    setDecodeEntropyThold: vi.fn<() => void>(),
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
    setHistoryDialogOpen: vi.fn<() => void>(),
    setSelectedHistoryTask: vi.fn<() => void>(),
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
      error: null,
      isError: false,
      isPending: false,
    },
    vadMaxSpeechDurationS: '',
    vadMinSilenceDurationMs: '',
    vadMinSpeechDurationMs: '',
    vadSamplesOverlap: '',
    vadSpeechPadMs: '',
    vadThreshold: '',
    webFileInputRef: { current: null },
    whisperVadModels: [],
  })),
}));

describe('media core flows e2e', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mediaHarnessState.audioRunning = false;
    mediaHarnessState.imageGenerating = false;
    mediaHarnessState.videoGenerating = false;
  });

  it('submits an image prompt and opens image history detail', async () => {
    await renderDesktopScene(<ImagePage />, { route: '/image' });

    await page.getByTestId('image-prompt-input').fill('Generate a calm lake');
    await page.getByTestId('image-generate-button').click();
    expect(mockImageSubmit).toHaveBeenCalled();

    await page.getByTestId('image-history-item-image-history-1').click();
    expect(mockImageHistoryDetail).toHaveBeenCalledWith('image-history-1');
  });

  it('submits a video prompt and opens video history detail', async () => {
    await renderDesktopScene(<VideoPage />, { route: '/video' });

    await page.getByTestId('video-prompt-input').fill('Generate a sunrise timelapse');
    await page.getByTestId('video-generate-button').click();
    expect(mockVideoSubmit).toHaveBeenCalled();

    await page.getByTestId('video-history-item-video-history-1').click();
    expect(mockVideoHistoryDetail).toHaveBeenCalledWith('video-history-1');
  });

  it('starts audio transcription and opens transcription history detail', async () => {
    await renderDesktopScene(<AudioPage />, { route: '/audio' });

    await page.getByTestId('audio-transcribe-button').click();
    expect(mockAudioTranscribe).toHaveBeenCalled();

    await page.getByTestId('audio-history-item-audio-history-1').click();
    expect(mockAudioHistoryDetail).toHaveBeenCalledWith('audio-history-1');
  });

  it('renders image progress and cancel controls while generating', async () => {
    mediaHarnessState.imageGenerating = true;
    await renderDesktopScene(<ImagePage />, { route: '/image' });

    await expect.element(page.getByTestId('image-generation-progress')).toBeVisible();
    await expect.element(page.getByTestId('image-generation-progress')).toHaveTextContent('42%');

    await page.getByTestId('image-cancel-button').click();
    expect(mockImageCancel).toHaveBeenCalled();
  });

  it('renders video progress and cancel controls while generating', async () => {
    mediaHarnessState.videoGenerating = true;
    await renderDesktopScene(<VideoPage />, { route: '/video' });

    await expect.element(page.getByTestId('video-generation-progress')).toBeVisible();
    await expect.element(page.getByTestId('video-generation-progress')).toHaveTextContent('64%');

    await page.getByTestId('video-cancel-button').click();
    expect(mockVideoCancel).toHaveBeenCalled();
  });

  it('renders audio progress and cancel controls while transcribing', async () => {
    mediaHarnessState.audioRunning = true;
    await renderDesktopScene(<AudioPage />, { route: '/audio' });

    await expect.element(page.getByTestId('audio-generation-progress')).toBeVisible();
    await expect.element(page.getByTestId('audio-generation-progress')).toHaveTextContent('35%');

    await page.getByTestId('audio-cancel-button').click();
    expect(mockAudioCancel).toHaveBeenCalled();
  });
});
