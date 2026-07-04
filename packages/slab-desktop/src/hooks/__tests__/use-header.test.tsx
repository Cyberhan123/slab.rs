import { render, screen } from '@testing-library/react';
import { Settings } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';

vi.mock('../../store/ui-state-storage', () => ({
  createUiStateStorage: () => ({
    getItem: vi.fn<() => Promise<null>>(async () => null),
    removeItem: vi.fn<() => Promise<void>>(async () => {}),
    setItem: vi.fn<() => Promise<void>>(async () => {}),
  }),
}));

import type { HeaderSearchControl, HeaderSelectControl } from '@/layouts/header';
import { HeaderProvider } from '@/layouts/header-provider';
import {
  useHeader,
  useHeaderControl,
  useHeaderSearch,
} from '../use-header';

const onControlChange = vi.fn<(value: string) => void>();
const onSearchChange = vi.fn<(value: string) => void>();
const selectControl = {
  type: 'select',
  value: 'model-a',
  options: [{ id: 'model-a', label: 'Model A' }],
  onValueChange: onControlChange,
} satisfies HeaderSelectControl;
const searchControl = {
  type: 'search',
  value: 'draft query',
  onValueChange: onSearchChange,
} satisfies HeaderSearchControl;

function HeaderProbe() {
  const { meta, control, search } = useHeader();

  return (
    <>
      <span data-testid="header-title">{meta.title}</span>
      <span data-testid="header-subtitle">{meta.subtitle}</span>
      <span data-testid="header-control">
        {control?.type === 'select' ? control.value : 'none'}
      </span>
      <span data-testid="header-search">
        {search?.type === 'search' ? search.value : 'none'}
      </span>
    </>
  );
}

function HeaderControlRegistration({ active }: { active: boolean }) {
  useHeaderControl(active ? selectControl : null);
  useHeaderSearch(active ? searchControl : null);

  return null;
}

describe('useHeader hooks', () => {
  it('reads route-owned header metadata from the provider default meta', () => {
    const defaultMeta = {
      icon: Settings,
      subtitle: 'Route subtitle',
      title: 'Route title',
    };

    render(
      <HeaderProvider defaultMeta={defaultMeta}>
        <HeaderProbe />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('header-title')).toHaveTextContent('Route title');
    expect(screen.getByTestId('header-subtitle')).toHaveTextContent('Route subtitle');
  });

  it('registers header control and search state, then clears them when inactive', () => {
    const { rerender } = render(
      <HeaderProvider>
        <HeaderControlRegistration active />
        <HeaderProbe />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('header-control')).toHaveTextContent('model-a');
    expect(screen.getByTestId('header-search')).toHaveTextContent('draft query');

    rerender(
      <HeaderProvider>
        <HeaderControlRegistration active={false} />
        <HeaderProbe />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('header-control')).toHaveTextContent('none');
    expect(screen.getByTestId('header-search')).toHaveTextContent('none');
  });
});
