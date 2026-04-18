import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import SetupPage from '@/pages/setup';
import type { SetupViewModel } from '@/pages/setup/hooks/use-setup';

import { renderDesktopScene } from '../test-utils';

const { mockUseSetup } = vi.hoisted(() => ({
  mockUseSetup: vi.fn<() => SetupViewModel>(),
}));

vi.mock('@/pages/setup/hooks/use-setup', () => ({
  useSetup: mockUseSetup,
}));

function createViewModel(overrides: Partial<SetupViewModel> = {}): SetupViewModel {
  return {
    setupStatus: {
      initialized: false,
      runtime_payload_installed: false,
      ffmpeg: {
        name: 'Ffmpeg',
        installed: false,
        version: null,
      },
      backends: [
        {
          name: 'Llama',
          installed: true,
          version: '1.0.0',
        },
        {
          name: 'Whisper',
          installed: false,
          version: null,
        },
      ],
    },
    isChecking: false,
    checkError: null,
    provisionState: 'running',
    provisionError: null,
    stageLabel: 'Downloading runtime payloads',
    stageHint: 'Step 2 of 4',
    progressPercent: 48,
    progressSummary: '48% complete',
    canRetry: false,
    handleRetry: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe('SetupPage browser visual regression', () => {
  beforeEach(() => {
    mockUseSetup.mockReset();
  });

  it('captures the running setup shell for visual regression coverage', async () => {
    mockUseSetup.mockReturnValue(createViewModel());

    await renderDesktopScene(<SetupPage />);

    await expect.element(
      page.getByRole('heading', { name: 'Slab is preparing your local runtime.' }),
    ).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'setup-page-running.png',
    );
  });

  it('captures the failure state and keeps retry interactions covered', async () => {
    const handleRetry = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);

    mockUseSetup.mockReturnValue(
      createViewModel({
        provisionState: 'failed',
        provisionError: 'FFmpeg download failed.',
        stageLabel: 'Setup failed',
        stageHint: 'Review the error below, then retry the setup task.',
        progressPercent: 72,
        progressSummary: 'Provisioning stopped before setup could complete.',
        canRetry: true,
        handleRetry,
      }),
    );

    await renderDesktopScene(<SetupPage />);

    const retryButton = page.getByRole('button', { name: 'Retry setup' });
    await expect.element(page.getByText('FFmpeg download failed.')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'setup-page-failed.png',
    );

    await retryButton.click();
    expect(handleRetry).toHaveBeenCalledOnce();
  });
});
