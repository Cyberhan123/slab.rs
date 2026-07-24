import { render, screen } from '@testing-library/react';
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
  it('renders nothing when there is no data or error', () => {
    const { container } = render(<ErrorDataDetail />);

    expect(container).toBeEmptyDOMElement();
  });

  it('describes a runtime_failure', () => {
    render(<ErrorDataDetail data={{ code: 'runtime_failure', runtime_code: 'boom' } as never} />);

    expect(screen.getByText('runtime_failure')).toBeInTheDocument();
    expect(screen.getByText('boom')).toBeInTheDocument();
  });

  it('describes an unsupported_chat_parameter', () => {
    render(<ErrorDataDetail data={{ code: 'unsupported_chat_parameter', param: 'temperature' } as never} />);

    expect(screen.getByText('Unsupported parameter: temperature')).toBeInTheDocument();
  });

  it('describes a model_download_unavailable with a suggestion', () => {
    render(
      <ErrorDataDetail
        data={{ code: 'model_download_unavailable', reason: 'no space', suggestion: 'free up disk' } as never}
      />,
    );

    expect(screen.getByText('no space. free up disk')).toBeInTheDocument();
  });

  it('derives the detail from an error via getErrorData', () => {
    render(<ErrorDataDetail error={new Error('upstream')} />);

    expect(screen.getByText('recovered from error')).toBeInTheDocument();
  });
});
