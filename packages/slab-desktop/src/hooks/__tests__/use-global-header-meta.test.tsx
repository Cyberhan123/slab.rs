import { render, screen } from '@testing-library/react';
import { Settings } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';

import { GlobalHeaderProvider } from '@/layouts/global-header-provider';
import type { HeaderSearchControl, HeaderSelectControl } from '@/layouts/header-controls';
import {
  useGlobalHeaderMeta,
  useGlobalHeaderState,
  usePageHeader,
  usePageHeaderControl,
  usePageHeaderSearch,
} from '../use-global-header-meta';

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

function HeaderMetaProbe() {
  const meta = useGlobalHeaderMeta();

  return (
    <>
      <span data-testid="header-title">{meta.title}</span>
      <span data-testid="header-subtitle">{meta.subtitle}</span>
    </>
  );
}

function HeaderStateProbe() {
  const { control, search } = useGlobalHeaderState();

  return (
    <>
      <span data-testid="header-control">
        {control?.type === 'select' ? control.value : 'none'}
      </span>
      <span data-testid="header-search">
        {search?.type === 'search' ? search.value : 'none'}
      </span>
    </>
  );
}

function PageHeaderRegistration({
  active,
  title,
}: {
  active: boolean;
  title: string;
}) {
  usePageHeader(
    active
      ? {
          icon: Settings,
          subtitle: `${title} subtitle`,
          title,
        }
      : null,
  );

  return null;
}

function PageHeaderControlRegistration({ active }: { active: boolean }) {
  usePageHeaderControl(active ? selectControl : null);
  usePageHeaderSearch(active ? searchControl : null);

  return null;
}

describe('global header hooks', () => {
  it('registers page metadata, updates it, and clears it when inactive', () => {
    const defaultMeta = {
      icon: Settings,
      subtitle: 'Default subtitle',
      title: 'Default',
    };
    const { rerender } = render(
      <GlobalHeaderProvider defaultMeta={defaultMeta}>
        <PageHeaderRegistration active title="Workspace" />
        <HeaderMetaProbe />
      </GlobalHeaderProvider>,
    );

    expect(screen.getByTestId('header-title')).toHaveTextContent('Workspace');
    expect(screen.getByTestId('header-subtitle')).toHaveTextContent('Workspace subtitle');

    rerender(
      <GlobalHeaderProvider defaultMeta={defaultMeta}>
        <PageHeaderRegistration active title="Settings" />
        <HeaderMetaProbe />
      </GlobalHeaderProvider>,
    );

    expect(screen.getByTestId('header-title')).toHaveTextContent('Settings');
    expect(screen.getByTestId('header-subtitle')).toHaveTextContent('Settings subtitle');

    rerender(
      <GlobalHeaderProvider defaultMeta={defaultMeta}>
        <PageHeaderRegistration active={false} title="Settings" />
        <HeaderMetaProbe />
      </GlobalHeaderProvider>,
    );

    expect(screen.getByTestId('header-title')).toHaveTextContent('Default');
    expect(screen.getByTestId('header-subtitle')).toHaveTextContent('Default subtitle');
  });

  it('registers header control and search state, then clears them when inactive', () => {
    const { rerender } = render(
      <GlobalHeaderProvider>
        <PageHeaderControlRegistration active />
        <HeaderStateProbe />
      </GlobalHeaderProvider>,
    );

    expect(screen.getByTestId('header-control')).toHaveTextContent('model-a');
    expect(screen.getByTestId('header-search')).toHaveTextContent('draft query');

    rerender(
      <GlobalHeaderProvider>
        <PageHeaderControlRegistration active={false} />
        <HeaderStateProbe />
      </GlobalHeaderProvider>,
    );

    expect(screen.getByTestId('header-control')).toHaveTextContent('none');
    expect(screen.getByTestId('header-search')).toHaveTextContent('none');
  });
});
