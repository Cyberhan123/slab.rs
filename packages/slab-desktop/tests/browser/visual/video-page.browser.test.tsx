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

function createVideoViewModel(overrides = {}) {
  const createVoidMock = () => vi.fn<(...args: unknown[]) => void>();
  const createAsyncVoidMock = () =>
    vi.fn<(...args: unknown[]) => Promise<void>>().mockResolvedValue(undefined);

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
    hasSelectedModel: true,
    immersivePreview: false,
    initImageDataUri: null,
    initImageInputRef: { current: null },
    isGenerating: false,
    negativePrompt: '',
    prompt: '',
    sampleMethod: 'euler_a',
    scheduler: 'normal',
    seed: -1,
    setAdvancedOpen: createVoidMock(),
    setCfgScale: createVoidMock(),
    setFps: createVoidMock(),
    setFrames: createVoidMock(),
    setGuidance: createVoidMock(),
    setHeightStr: createVoidMock(),
    setImmersivePreview: createVoidMock(),
    setInitImageDataUri: createVoidMock(),
    setNegativePrompt: createVoidMock(),
    setPrompt: createVoidMock(),
    setSampleMethod: createVoidMock(),
    setScheduler: createVoidMock(),
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
        videoPath: '/path/to/generated/video.mp4',
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
