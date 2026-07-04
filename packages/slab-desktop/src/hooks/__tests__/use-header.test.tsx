import { render, screen } from '@testing-library/react';
import { Settings } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';

import type { HeaderSearchConfig, HeaderSelectConfig } from '@/layouts/header';
import { HeaderProvider } from '@/layouts/header-provider';
import { useHeader } from '../use-header';

const onSelectChange = vi.fn<(value: string) => void>();
const onSearchChange = vi.fn<(value: string) => void>();
const selectConfig = {
  value: 'model-a',
  options: [{ id: 'model-a', label: 'Model A' }],
  onChange: onSelectChange,
} satisfies HeaderSelectConfig;
const searchConfig = {
  value: 'draft query',
  onChange: onSearchChange,
} satisfies HeaderSearchConfig;

function HeaderProbe() {
  const { meta, select, search } = useHeader();

  return (
    <>
      <span data-testid="header-title">{meta.title}</span>
      <span data-testid="header-subtitle">{meta.subtitle}</span>
      <span data-testid="header-context">{meta.contextLabel ?? 'none'}</span>
      <span data-testid="header-select">{select?.value ?? 'none'}</span>
      <span data-testid="header-search">{search?.value ?? 'none'}</span>
    </>
  );
}

function HeaderRegistration({ active }: { active: boolean }) {
  useHeader(
    active
      ? {
          meta: {
            icon: Settings,
            title: 'Registered title',
            subtitle: 'Registered subtitle',
            contextLabel: 'Registered context',
          },
          select: selectConfig,
          search: searchConfig,
        }
      : null,
  );

  return null;
}

function SelectRegistration({ value }: { value: string }) {
  useHeader({
    select: {
      ...selectConfig,
      value,
    },
  });

  return null;
}

describe('useHeader', () => {
  it('reads route-owned header metadata from the provider default meta', () => {
    const defaultMeta = {
      icon: Settings,
      subtitle: 'Route subtitle',
      title: 'Route title',
      contextLabel: 'Route context',
    };

    render(
      <HeaderProvider defaultMeta={defaultMeta}>
        <HeaderProbe />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('header-title')).toHaveTextContent('Route title');
    expect(screen.getByTestId('header-subtitle')).toHaveTextContent('Route subtitle');
    expect(screen.getByTestId('header-context')).toHaveTextContent('Route context');
  });

  it('registers header metadata, select, and search, then clears them when inactive', () => {
    const { rerender } = render(
      <HeaderProvider>
        <HeaderRegistration active />
        <HeaderProbe />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('header-title')).toHaveTextContent('Registered title');
    expect(screen.getByTestId('header-select')).toHaveTextContent('model-a');
    expect(screen.getByTestId('header-search')).toHaveTextContent('draft query');

    rerender(
      <HeaderProvider>
        <HeaderRegistration active={false} />
        <HeaderProbe />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('header-title')).toHaveTextContent('Slab');
    expect(screen.getByTestId('header-select')).toHaveTextContent('none');
    expect(screen.getByTestId('header-search')).toHaveTextContent('none');
  });

  it('uses the latest registration while preserving earlier entries', () => {
    const { unmount } = render(
      <HeaderProvider>
        <SelectRegistration value="model-a" />
        <SelectRegistration value="model-b" />
        <HeaderProbe />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('header-select')).toHaveTextContent('model-b');
    unmount();
  });
});
