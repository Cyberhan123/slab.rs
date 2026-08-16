import { render } from 'vitest-browser-react';
import { describe, expect, it, vi } from 'vitest';

import { GenerationProgressView } from '../generation-progress';

vi.mock('@slab/components/progress', () => ({
  Progress: ({ value }: { value?: number }) => <div data-testid="progress" data-value={value} />,
}));

const labels = {
  eta: 'ETA',
  finalizing: 'Finalizing',
  queued: 'Queued',
  running: 'Running',
  step: 'Step',
  title: 'Generating',
};

describe('GenerationProgressView', () => {
  it('renders nothing when progress is null', async () => {
    const { container } = await render(<GenerationProgressView progress={null} labels={labels} />);

    expect(container.childNodes.length).toBe(0);
  });

  it('renders the running stage label and rounded percent', async () => {
    const screen = await render(
      <GenerationProgressView
        progress={{ percent: 42.6, stage: 'running', stepLabel: 'denoising', etaMs: 5000 } as never}
        labels={labels}
      />,
    );

    await expect.element(screen.getByText('Generating')).toBeInTheDocument();
    await expect.element(screen.getByText('43%')).toBeInTheDocument();
    await expect.element(screen.getByText('Running · denoising')).toBeInTheDocument();
    await expect.element(screen.getByText('ETA: 5s')).toBeInTheDocument();
  });

  it('renders the queued and finalizing stage labels', async () => {
    const screen = await render(
      <GenerationProgressView progress={{ stage: 'queued' } as never} labels={labels} />,
    );
    await expect.element(screen.getByText('Queued')).toBeInTheDocument();

    await screen.rerender(<GenerationProgressView progress={{ stage: 'finalizing' } as never} labels={labels} />);
    await expect.element(screen.getByText('Finalizing')).toBeInTheDocument();
  });

  it('shows an em-dash ETA when etaMs is null', async () => {
    const screen = await render(
      <GenerationProgressView progress={{ stage: 'running', etaMs: null } as never} labels={labels} />,
    );

    await expect.element(screen.getByText('ETA: —')).toBeInTheDocument();
  });
});
