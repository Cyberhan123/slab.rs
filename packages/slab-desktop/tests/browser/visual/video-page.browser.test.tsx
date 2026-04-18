import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import VideoPage from '@/pages/video';
import { renderDesktopScene } from '../test-utils';

const { mockUseVideoGeneration } = vi.hoisted(() => ({
  mockUseVideoGeneration: vi.fn<
    () => Partial<
      ReturnType<typeof import('@/pages/video/hooks/use-video-generation').useVideoGeneration>
    >
  >(),
}));

vi.mock('@/pages/video/hooks/use-video-generation', () => ({
  useVideoGeneration: mockUseVideoGeneration,
}));

vi.mock('@/hooks/use-persisted-header-select', () => ({
  usePersistedHeaderSelect: vi.fn<() => void>(() => ({ value: 'model-1', setValue: vi.fn<() => void>() })),
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

function createVideoViewModel(overrides = {}) {
  return {
    advancedOpen: false,
    cfgScale: 7,
    footerHint: 'Clip duration: 2.0s',
    fps: 8,
    frames: 16,
    guidance: 3.5,
    handleCancel: vi.fn<() => void>().mockResolvedValue(undefined),
    handleDownload: vi.fn<() => void>(),
    handleInitImageChange: vi.fn<() => void>(),
    handleInitImageDrop: vi.fn<() => void>(),
    handleSubmit: vi.fn<() => void>().mockResolvedValue(undefined),
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
    setAdvancedOpen: vi.fn<() => void>(),
    setCfgScale: vi.fn<() => void>(),
    setFps: vi.fn<() => void>(),
    setFrames: vi.fn<() => void>(),
    setGuidance: vi.fn<() => void>(),
    setHeightStr: vi.fn<() => void>(),
    setImmersivePreview: vi.fn<() => void>(),
    setInitImageDataUri: vi.fn<() => void>(),
    setNegativePrompt: vi.fn<() => void>(),
    setPrompt: vi.fn<() => void>(),
    setSampleMethod: vi.fn<() => void>(),
    setScheduler: vi.fn<() => void>(),
    setSeed: vi.fn<() => void>(),
    setSteps: vi.fn<() => void>(),
    setStrength: vi.fn<() => void>(),
    setWidthStr: vi.fn<() => void>(),
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
