import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import VideoPage from '@/pages/video';
import { renderDesktopScene } from '../test-utils';

const { mockUseVideoGeneration } = vi.hoisted(() => ({
  mockUseVideoGeneration: vi.fn<() => unknown>(),
}));

vi.mock('@/pages/video/hooks/use-video-generation', () => ({
  useVideoGeneration: mockUseVideoGeneration,
}));

vi.mock('@/hooks/use-persisted-header-select', () => ({
  usePersistedHeaderSelect: vi.fn<() => unknown>(() => ({
    value: 'model-1',
    setValue: vi.fn<() => void>(),
  })),
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

const createVoidMock = () => vi.fn<(...args: unknown[]) => void>();
const createAsyncVoidMock = () =>
  vi.fn<(...args: unknown[]) => Promise<void>>().mockResolvedValue(undefined);

function createVideoViewModel(overrides = {}) {
  return {
    advancedOpen: false,
    cfgScale: 7,
    footerHint: 'Clip duration: 2.0s',
    fps: 8,
    frames: 16,
    guidance: 3.5,
    handleCancel: createAsyncVoidMock(),
    handleDownload: createVoidMock(),
    handleInitImageChange: createVoidMock(),
    handleInitImageDrop: createVoidMock(),
    handleSubmit: createAsyncVoidMock(),
    heightStr: '512',
    heightValue: 512,
    history: [],
    historyDialogOpen: false,
    historyError: null,
    historyLoading: false,
    hasSelectedModel: true,
    immersivePreview: false,
    initImageDataUri: null,
    initImageInputRef: { current: null },
    isGenerating: false,
    negativePrompt: '',
    openHistoryDetail: createAsyncVoidMock(),
    prompt: '',
    sampleMethod: 'euler_a',
    scheduler: 'normal',
    seed: -1,
    selectedHistoryTask: null,
    setAdvancedOpen: createVoidMock(),
    setCfgScale: createVoidMock(),
    setFps: createVoidMock(),
    setFrames: createVoidMock(),
    setGuidance: createVoidMock(),
    setHeightStr: createVoidMock(),
    setHistoryDialogOpen: createVoidMock(),
    setImmersivePreview: createVoidMock(),
    setInitImageDataUri: createVoidMock(),
    setNegativePrompt: createVoidMock(),
    setPrompt: createVoidMock(),
    setSampleMethod: createVoidMock(),
    setScheduler: createVoidMock(),
    setSelectedHistoryTask: createVoidMock(),
    setSeed: createVoidMock(),
    setSteps: createVoidMock(),
    setStrength: createVoidMock(),
    setWidthStr: createVoidMock(),
    stageDescription: 'Enter a prompt to generate video',
    stageStatus: 'Awaiting prompt',
    stageTitle: 'Ready',
    steps: 20,
    strength: 0.75,
    videoPath: null,
    widthStr: '512',
    widthValue: 512,
    ...overrides,
  };
}

const stableChromiumScreenshot = {
  comparatorName: 'pixelmatch' as const,
  comparatorOptions: {
    allowedMismatchedPixels: 100,
  },
};
const stableVideoDataUrl =
  'data:video/webm;base64,GkXfo59ChoEBQveBAULygQRC84EIQoKEd2VibUKHgQJChYECGFOAZwEAAAAAAAHpEU2bdLpNu4tTq4QVSalmU6yBoU27i1OrhBZUrmtTrIHWTbuMU6uEElTDZ1OsggEjTbuMU6uEHFO7a1OsggHT7AEAAAAAAABZAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAVSalmsCrXsYMPQkBNgIxMYXZmNjIuMy4xMDBXQYxMYXZmNjIuMy4xMDBEiYhAj0AAAAAAABZUrmvIrgEAAAAAAAA/14EBc8WI+BA4Ec5PAOWcgQAitZyDdW5kiIEAhoVWX1ZQOYOBASPjg4Q7msoA4JCwgRC6gRCagQJVsIRVuYEBElTDZ0B/c3OfY8CAZ8iZRaOHRU5DT0RFUkSHjExhdmY2Mi4zLjEwMHNz2mPAi2PFiPgQOBHOTwDlZ8ilRaOHRU5DT0RFUkSHmExhdmM2Mi4xMS4xMDAgbGlidnB4LXZwOWfIoUWjiERVUkFUSU9ORIeTMDA6MDA6MDEuMDAwMDAwMDAwAB9DtnWm54EAo6GBAACAgkmDQgAA8AD2ADgkHBhCAAAwYAAAEL///YsqAAAcU7trkbuPs4EAt4r3gQHxggGo8IED';

describe('VideoPage browser visual regression', () => {
  beforeEach(() => {
    mockUseVideoGeneration.mockReset();
  });

  it('captures the video workbench idle state', async () => {
    mockUseVideoGeneration.mockReturnValue(createVideoViewModel());

    await renderDesktopScene(<VideoPage />, { route: '/video' });

    await expect.element(page.getByText('Enter a prompt to generate video')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'video-page-idle.png',
    );
  });

  it('captures the video workbench with prompt entered', async () => {
    mockUseVideoGeneration.mockReturnValue(
      createVideoViewModel({
        prompt: 'A calming ocean wave crashing on the shore',
        stageDescription: 'Generating 16 frames at 8 fps',
        stageStatus: 'Queued',
        stageTitle: 'Ready',
      }),
    );

    await renderDesktopScene(<VideoPage />, { route: '/video' });

    await expect
      .element(page.getByText('A calming ocean wave crashing on the shore'))
      .toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'video-page-with-prompt.png',
      stableChromiumScreenshot,
    );
  });

  it('captures the video workbench generating state', async () => {
    mockUseVideoGeneration.mockReturnValue(
      createVideoViewModel({
        prompt: 'A bird flying through clouds',
        isGenerating: true,
        stageDescription: 'Generating 16 frames at 8 fps',
        stageStatus: 'Rendering',
        stageTitle: 'Rendering',
      }),
    );

    await renderDesktopScene(<VideoPage />, { route: '/video' });

    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'video-page-generating.png',
    );
  });

  it('captures the video workbench with completed video', async () => {
    mockUseVideoGeneration.mockReturnValue(
      createVideoViewModel({
        prompt: 'A bird flying through clouds',
        videoPath: stableVideoDataUrl,
        stageDescription: 'Video generation complete',
        stageStatus: 'Ready',
        stageTitle: 'Ready',
      }),
    );

    await renderDesktopScene(<VideoPage />, { route: '/video' });

    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'video-page-completed.png',
      stableChromiumScreenshot,
    );
  });
});
