import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import ImagePage from '@/pages/image';
import type { GeneratedImage } from '@/pages/image/const';
import { renderDesktopScene } from '../test-utils';

const { mockUseImageGeneration } = vi.hoisted(() => ({
  mockUseImageGeneration: vi.fn<
    () => Partial<
      ReturnType<typeof import('@/pages/image/hooks/use-image-generation').useImageGeneration>
    >
  >(),
}));

vi.mock('@/pages/image/hooks/use-image-generation', () => ({
  useImageGeneration: mockUseImageGeneration,
}));

vi.mock('@/pages/image/hooks/use-image-generation-controls', () => ({
  useImageGenerationControls: vi.fn<() => void>(() => ({
    activeDimensionPreset: '512x512',
    advancedOpen: false,
    cfgScale: 7,
    clipSkip: 0,
    eta: 0,
    guidance: 3.5,
    handleDimensionPreset: vi.fn<() => void>(),
    heightStr: '512',
    isResolvingModelState: false,
    mode: 'txt2img',
    numImages: 1,
    parsedHeight: 512,
    parsedWidth: 512,
    sampleMethod: 'euler_a',
    scheduler: 'normal',
    seed: -1,
    setAdvancedOpen: vi.fn<() => void>(),
    setCfgScale: vi.fn<() => void>(),
    setClipSkip: vi.fn<() => void>(),
    setEta: vi.fn<() => void>(),
    setGuidance: vi.fn<() => void>(),
    setHeightStr: vi.fn<() => void>(),
    setMode: vi.fn<() => void>(),
    setNumImages: vi.fn<() => void>(),
    setSampleMethod: vi.fn<() => void>(),
    setScheduler: vi.fn<() => void>(),
    setSeed: vi.fn<() => void>(),
    setSteps: vi.fn<() => void>(),
    setStrength: vi.fn<() => void>(),
    setWidthStr: vi.fn<() => void>(),
    steps: 20,
    strength: 0.75,
    widthStr: '512',
  })),
}));

vi.mock('@/pages/image/hooks/use-image-model-preparation', () => ({
  useImageModelPreparation: vi.fn<() => void>(() => ({
    catalogLoading: false,
    isPreparingModel: false,
    modelOptions: [
      { id: 'model-1', label: 'Stable Diffusion 1.5', downloaded: true, local_path: '/path/to/model' },
    ],
    prepareSelectedModel: vi.fn<() => void>().mockResolvedValue('/path/to/model'),
    selectedModelId: 'model-1',
    setSelectedModelId: vi.fn<() => void>(),
  })),
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

function createImageViewModel(overrides = {}) {
  return {
    activeDimensionPreset: '512x512',
    advancedOpen: false,
    cfgScale: 7,
    clipSkip: 0,
    eta: 0,
    guidance: 3.5,
    handleCancel: vi.fn<() => void>().mockResolvedValue(undefined),
    handleDimensionPreset: vi.fn<() => void>(),
    handleDownload: vi.fn<() => void>(),
    handleInitImageChange: vi.fn<() => void>(),
    handleSubmit: vi.fn<() => void>().mockResolvedValue(undefined),
    heightStr: '512',
    images: [] as GeneratedImage[],
    initImageDataUri: null,
    initImageInputRef: { current: null },
    isBusy: false,
    isGenerating: false,
    isPreparingModel: false,
    isResolvingModelState: false,
    mode: 'txt2img' as const,
    negativePrompt: '',
    numImages: 1,
    parsedHeight: 512,
    parsedWidth: 512,
    prompt: '',
    sampleMethod: 'euler_a',
    scheduler: 'normal',
    seed: -1,
    selectedModelId: 'model-1',
    setAdvancedOpen: vi.fn<() => void>(),
    setCfgScale: vi.fn<() => void>(),
    setClipSkip: vi.fn<() => void>(),
    setEta: vi.fn<() => void>(),
    setGuidance: vi.fn<() => void>(),
    setHeightStr: vi.fn<() => void>(),
    setInitImageDataUri: vi.fn<() => void>(),
    setMode: vi.fn<() => void>(),
    setNegativePrompt: vi.fn<() => void>(),
    setNumImages: vi.fn<() => void>(),
    setPrompt: vi.fn<() => void>(),
    setSampleMethod: vi.fn<() => void>(),
    setScheduler: vi.fn<() => void>(),
    setSeed: vi.fn<() => void>(),
    setSteps: vi.fn<() => void>(),
    setStrength: vi.fn<() => void>(),
    setWidthStr: vi.fn<() => void>(),
    setZoomedImage: vi.fn<() => void>(),
    steps: 20,
    strength: 0.75,
    widthStr: '512',
    zoomedImage: null,
    ...overrides,
  };
}

const stableChromiumScreenshot = {
  comparatorName: 'pixelmatch' as const,
  comparatorOptions: {
    allowedMismatchedPixels: 100,
  },
};

describe('ImagePage browser visual regression', () => {
  beforeEach(() => {
    mockUseImageGeneration.mockReset();
  });

  it('captures the image workbench idle state', async () => {
    mockUseImageGeneration.mockReturnValue(createImageViewModel());

    await renderDesktopScene(<ImagePage />, { route: '/image' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'image-page-idle.png',
      stableChromiumScreenshot,
    );
  });

  it('captures the image workbench with prompt entered', async () => {
    mockUseImageGeneration.mockReturnValue(
      createImageViewModel({
        prompt: 'A serene landscape with mountains and a lake at sunset',
      }),
    );

    await renderDesktopScene(<ImagePage />, { route: '/image' });

    await expect
      .element(page.getByTestId('desktop-browser-scene'))
      .toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'image-page-with-prompt.png',
      stableChromiumScreenshot,
    );
  });

  it('captures the image workbench generating state', async () => {
    mockUseImageGeneration.mockReturnValue(
      createImageViewModel({
        prompt: 'A futuristic cityscape',
        isGenerating: true,
        isBusy: true,
      }),
    );

    await renderDesktopScene(<ImagePage />, { route: '/image' });

    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'image-page-generating.png',
      stableChromiumScreenshot,
    );
  });

  it('captures the image workbench with generated images', async () => {
    const mockImages: GeneratedImage[] = [
      {
        src: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
        prompt: 'A futuristic cityscape',
        width: 512,
        height: 512,
        mode: 'txt2img',
      },
      {
        src: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
        prompt: 'A futuristic cityscape',
        width: 512,
        height: 512,
        mode: 'txt2img',
      },
    ];

    mockUseImageGeneration.mockReturnValue(
      createImageViewModel({
        prompt: 'A futuristic cityscape',
        images: mockImages,
      }),
    );

    await renderDesktopScene(<ImagePage />, { route: '/image' });

    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'image-page-with-results.png',
      stableChromiumScreenshot,
    );
  });
});
