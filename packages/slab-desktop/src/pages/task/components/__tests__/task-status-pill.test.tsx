import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { renderStatusPill } from '../task-status-pill';

const t = (key: string) => key;

describe('renderStatusPill', () => {
  it('renders the translated label for a known status', () => {
    const { container } = render(<>{renderStatusPill('failed', t)}</>);

    expect(screen.getByText('pages.task.status.failed')).toBeInTheDocument();
    expect(container.querySelector('span.rounded-full')).not.toBeNull();
  });

  it('renders each supported status without crashing', () => {
    const statuses = ['succeeded', 'running', 'pending', 'cancelled', 'interrupted', 'failed'];
    for (const status of statuses) {
      const { unmount } = render(<>{renderStatusPill(status, t)}</>);
      expect(screen.getByText(`pages.task.status.${status}`)).toBeInTheDocument();
      unmount();
    }
  });

  it('falls back to the raw status string for an unknown status', () => {
    render(<>{renderStatusPill('something-weird', t)}</>);

    expect(screen.getByText('something-weird')).toBeInTheDocument();
  });
});
