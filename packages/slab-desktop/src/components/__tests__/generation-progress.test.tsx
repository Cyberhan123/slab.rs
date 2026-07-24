import { render, screen } from '@testing-library/react';
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
  it('renders nothing when progress is null', () => {
    const { container } = render(<GenerationProgressView progress={null} labels={labels} />);

    expect(container).toBeEmptyDOMElement();
  });

  it('renders the running stage label and rounded percent', () => {
    render(
      <GenerationProgressView
        progress={{ percent: 42.6, stage: 'running', stepLabel: 'denoising', etaMs: 5000 } as never}
        labels={labels}
      />,
    );

    expect(screen.getByText('Generating')).toBeInTheDocument();
    expect(screen.getByText('43%')).toBeInTheDocument();
    expect(screen.getByText('Running · denoising')).toBeInTheDocument();
    expect(screen.getByText('ETA: 5s')).toBeInTheDocument();
  });

  it('renders the queued and finalizing stage labels', () => {
    const { rerender } = render(
      <GenerationProgressView progress={{ stage: 'queued' } as never} labels={labels} />,
    );
    expect(screen.getByText('Queued')).toBeInTheDocument();

    rerender(<GenerationProgressView progress={{ stage: 'finalizing' } as never} labels={labels} />);
    expect(screen.getByText('Finalizing')).toBeInTheDocument();
  });

  it('shows an em-dash ETA when etaMs is null', () => {
    render(
      <GenerationProgressView progress={{ stage: 'running', etaMs: null } as never} labels={labels} />,
    );

    expect(screen.getByText('ETA: —')).toBeInTheDocument();
  });
});
