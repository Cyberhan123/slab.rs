import { render, screen } from '@testing-library/react';
import { Boxes } from 'lucide-react';
import type { ReactNode } from 'react';
import { describe, expect, it } from 'vitest';

import { TaskMetricCard } from '../task-metric-card';

describe('TaskMetricCard', () => {
  it('renders the label, value, note and children', () => {
    render(
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

    expect(screen.getByText('Tasks')).toBeInTheDocument();
    expect(screen.getByText('12')).toBeInTheDocument();
    expect(screen.getByText('+2 today')).toBeInTheDocument();
    expect(screen.getByTestId('sparkline')).toBeInTheDocument();
  });

  it.each([
    ['success', 'text-success'],
    ['danger', 'text-destructive'],
    ['muted', 'text-muted-foreground'],
  ] as const)('maps noteTone %s to the right class', (noteTone, expected) => {
    render(
      <TaskMetricCard label="L" value="1" note="n" noteTone={noteTone} icon={Boxes}>
        {null as ReactNode}
      </TaskMetricCard>,
    );

    expect(screen.getByText('n').className).toContain(expected);
  });
});
