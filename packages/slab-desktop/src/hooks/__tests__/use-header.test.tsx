import { render, screen } from '@testing-library/react';
import { Settings } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';

import { GlobalHeaderProvider } from '@/layouts/global-header-provider';
import type { HeaderSearchControl, HeaderSelectControl } from '@/layouts/header-controls';
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
      <GlobalHeaderProvider defaultMeta={defaultMeta}>
        <HeaderProbe />
      </GlobalHeaderProvider>,
    );

    expect(screen.getByTestId('header-title')).toHaveTextContent('Route title');
    expect(screen.getByTestId('header-subtitle')).toHaveTextContent('Route subtitle');
  });

  it('registers header control and search state, then clears them when inactive', () => {
    const { rerender } = render(
      <GlobalHeaderProvider>
        <HeaderControlRegistration active />
        <HeaderProbe />
      </GlobalHeaderProvider>,
    );

    expect(screen.getByTestId('header-control')).toHaveTextContent('model-a');
    expect(screen.getByTestId('header-search')).toHaveTextContent('draft query');

    rerender(
      <GlobalHeaderProvider>
        <HeaderControlRegistration active={false} />
        <HeaderProbe />
      </GlobalHeaderProvider>,
    );

    expect(screen.getByTestId('header-control')).toHaveTextContent('none');
    expect(screen.getByTestId('header-search')).toHaveTextContent('none');
  });
});
