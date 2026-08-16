import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-react';

import { renderStatusPill } from '../task-status-pill';

const t = (key: string) => key;

describe('renderStatusPill', () => {
  it('renders the translated label for a known status', async () => {
    const screen = await render(<>{renderStatusPill('failed', t)}</>);

    await expect.element(screen.getByText('pages.task.status.failed')).toBeInTheDocument();
    expect(screen.container.querySelector('span.rounded-full')).not.toBeNull();
  });

  it('renders each supported status without crashing', async () => {
    const statuses = ['succeeded', 'running', 'pending', 'cancelled', 'interrupted', 'failed'];
    for (const status of statuses) {
      const screen = await render(<>{renderStatusPill(status, t)}</>);
      await expect.element(screen.getByText(`pages.task.status.${status}`)).toBeInTheDocument();
      await screen.unmount();
    }
  });

  it('falls back to the raw status string for an unknown status', async () => {
    const screen = await render(<>{renderStatusPill('something-weird', t)}</>);

    await expect.element(screen.getByText('something-weird')).toBeInTheDocument();
  });
});
