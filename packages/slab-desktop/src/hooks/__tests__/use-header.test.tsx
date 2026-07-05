import { fireEvent, render, screen } from '@testing-library/react';
import { memo } from 'react';
import { Settings } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';

import type { HeaderHistoryConfig, HeaderSearchConfig, HeaderSelectConfig } from '@/layouts/header';
import Header from '@/layouts/header';
import { HeaderProvider } from '@/layouts/header-provider';
import { useHeader } from '../use-header';

const onSelectChange = vi.fn<(value: string) => void>();
const onSearchChange = vi.fn<(value: string) => void>();
const onHistoryClick = vi.fn<() => void>();
const selectConfig = {
  value: 'model-a',
  options: [{ id: 'model-a', label: 'Model A' }],
  onChange: onSelectChange,
} satisfies HeaderSelectConfig;
const searchConfig = {
  value: 'draft query',
  onChange: onSearchChange,
} satisfies HeaderSearchConfig;
const historyConfig = {
  ariaLabel: 'Open history',
  onClick: onHistoryClick,
  title: 'History',
} satisfies HeaderHistoryConfig;

function HeaderProbe() {
  const { history, meta, select, search } = useHeader();

  return (
    <>
      <span data-testid="header-title">{meta.title}</span>
      <span data-testid="header-subtitle">{meta.subtitle}</span>
      <span data-testid="header-context">{meta.contextLabel ?? 'none'}</span>
      <span data-testid="header-history">{history?.title ?? 'none'}</span>
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
          history: historyConfig,
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

const HeaderRenderProbe = memo(function HeaderRenderProbe({
  onRender,
}: {
  onRender: (history: HeaderHistoryConfig | null) => void;
}) {
  const { history } = useHeader();
  onRender(history);
  return <span data-testid="header-render-probe">{history?.title ?? 'none'}</span>;
});

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
    expect(screen.getByTestId('header-history')).toHaveTextContent('History');
    expect(screen.getByTestId('header-select')).toHaveTextContent('model-a');
    expect(screen.getByTestId('header-search')).toHaveTextContent('draft query');

    rerender(
      <HeaderProvider>
        <HeaderRegistration active={false} />
        <HeaderProbe />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('header-title')).toHaveTextContent('Slab');
    expect(screen.getByTestId('header-history')).toHaveTextContent('none');
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

  it('renders registered history as an actionable header button', () => {
    onHistoryClick.mockClear();

    render(
      <HeaderProvider>
        <HeaderRegistration active />
        <Header />
      </HeaderProvider>,
    );

    const historyButton = screen.getByTestId('header-history-control');
    expect(historyButton).toHaveAttribute('aria-label', 'Open history');

    fireEvent.click(historyButton);

    expect(onHistoryClick).toHaveBeenCalledTimes(1);
  });

  it('disables registered history actions while preserving the control', () => {
    onHistoryClick.mockClear();

    function DisabledHistoryRegistration() {
      useHeader({
        history: {
          ...historyConfig,
          disabled: true,
        },
      });

      return null;
    }

    render(
      <HeaderProvider>
        <DisabledHistoryRegistration />
        <Header />
      </HeaderProvider>,
    );

    const historyButton = screen.getByTestId('header-history-control');
    expect(historyButton).toBeDisabled();

    fireEvent.click(historyButton);

    expect(onHistoryClick).not.toHaveBeenCalled();
  });

  it('removes the history button after registration cleanup', () => {
    const { rerender } = render(
      <HeaderProvider>
        <HeaderRegistration active />
        <Header />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('header-history-control')).toBeInTheDocument();

    rerender(
      <HeaderProvider>
        <HeaderRegistration active={false} />
        <Header />
      </HeaderProvider>,
    );

    expect(screen.queryByTestId('header-history-control')).not.toBeInTheDocument();
  });

  it('does not republish equivalent history registrations', () => {
    const onRender = vi.fn<(history: HeaderHistoryConfig | null) => void>();

    function StableHistoryRegistration({ value }: { value: string }) {
      useHeader({
        history: historyConfig,
      });

      return <span data-testid="stable-value">{value}</span>;
    }

    const { rerender } = render(
      <HeaderProvider>
        <StableHistoryRegistration value="first" />
        <HeaderRenderProbe onRender={onRender} />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('header-render-probe')).toHaveTextContent('History');
    expect(onRender).toHaveBeenCalledTimes(2);

    rerender(
      <HeaderProvider>
        <StableHistoryRegistration value="second" />
        <HeaderRenderProbe onRender={onRender} />
      </HeaderProvider>,
    );

    expect(screen.getByTestId('stable-value')).toHaveTextContent('second');
    expect(onRender).toHaveBeenCalledTimes(2);
  });
});
