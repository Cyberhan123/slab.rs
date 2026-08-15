import { render } from 'vitest-browser-react';
import { describe, expect, it, vi } from 'vitest';

import { ErrorDataDetail } from '../error-data-detail';

vi.mock('@slab/api', () => ({
  getErrorData: vi.fn<(error: unknown) => unknown>((error) =>
    error
      ? { code: 'runtime_failure', runtime_code: 'recovered from error' }
      : null,
  ),
}));

describe('ErrorDataDetail', () => {
  it('renders nothing when there is no data or error', async () => {
    const { container } = await render(<ErrorDataDetail />);

    expect(container.childNodes.length).toBe(0);
  });

  it('describes a runtime_failure', async () => {
    const screen = await render(<ErrorDataDetail data={{ code: 'runtime_failure', runtime_code: 'boom' } as never} />);

    await expect.element(screen.getByText('runtime_failure')).toBeInTheDocument();
    await expect.element(screen.getByText('boom')).toBeInTheDocument();
  });

  it('describes an unsupported_chat_parameter', async () => {
    const screen = await render(<ErrorDataDetail data={{ code: 'unsupported_chat_parameter', param: 'temperature' } as never} />);

    await expect.element(screen.getByText('Unsupported parameter: temperature')).toBeInTheDocument();
  });

  it('describes a model_download_unavailable with a suggestion', async () => {
    const screen = await render(
      <ErrorDataDetail
        data={{ code: 'model_download_unavailable', reason: 'no space', suggestion: 'free up disk' } as never}
      />,
    );

    await expect.element(screen.getByText('no space. free up disk')).toBeInTheDocument();
  });

  it('derives the detail from an error via getErrorData', async () => {
    const screen = await render(<ErrorDataDetail error={new Error('upstream')} />);

    await expect.element(screen.getByText('recovered from error')).toBeInTheDocument();
  });
});
