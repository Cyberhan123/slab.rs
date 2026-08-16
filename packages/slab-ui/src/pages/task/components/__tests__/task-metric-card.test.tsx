import { Boxes } from 'lucide-react';
import type { ReactNode } from 'react';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-react';

import { TaskMetricCard } from '../task-metric-card';

describe('TaskMetricCard', () => {
  it('renders the label, value, note and children', async () => {
    const screen = await render(
      <TaskMetricCard
        label="Tasks"
        value="12"
        note="+2 today"
        noteTone="success"
        icon={Boxes}
      >
        <span data-testid="sparkline">chart</span>
      </TaskMetricCard>,
    );

    await expect.element(screen.getByText('Tasks')).toBeInTheDocument();
    await expect.element(screen.getByText('12')).toBeInTheDocument();
    await expect.element(screen.getByText('+2 today')).toBeInTheDocument();
    await expect.element(screen.getByTestId('sparkline')).toBeInTheDocument();
  });

  it.each([
    ['success', 'text-success'],
    ['danger', 'text-destructive'],
    ['muted', 'text-muted-foreground'],
  ] as const)('maps noteTone %s to the right class', async (noteTone, expected) => {
    const screen = await render(
      <TaskMetricCard label="L" value="1" note="n" noteTone={noteTone} icon={Boxes}>
        {null as ReactNode}
      </TaskMetricCard>,
    );

    expect(screen.getByText('n').element()?.className).toContain(expected);
  });
});
