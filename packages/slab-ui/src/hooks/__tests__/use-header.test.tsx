import { memo } from 'react';
import { Settings } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import type { HeaderHistoryConfig, HeaderSearchConfig, HeaderSelectConfig } from '@slab/ui/layouts/header';
import Header from '@slab/ui/layouts/header';
import { HeaderProvider } from '@slab/ui/layouts/header-provider';
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
  it('reads route-owned header metadata from the provider default meta', async () => {
    const defaultMeta = {
      icon: Settings,
      subtitle: 'Route subtitle',
      title: 'Route title',
      contextLabel: 'Route context',
    };

    const screen = await render(
      <HeaderProvider defaultMeta={defaultMeta}>
        <HeaderProbe />
      </HeaderProvider>,
    );

    await expect.element(screen.getByTestId('header-title')).toHaveTextContent('Route title');
    await expect.element(screen.getByTestId('header-subtitle')).toHaveTextContent('Route subtitle');
    await expect.element(screen.getByTestId('header-context')).toHaveTextContent('Route context');
  });

  it('registers header metadata, select, and search, then clears them when inactive', async () => {
    const screen = await render(
      <HeaderProvider>
        <HeaderRegistration active />
        <HeaderProbe />
      </HeaderProvider>,
    );

    await expect.element(screen.getByTestId('header-title')).toHaveTextContent('Registered title');
    await expect.element(screen.getByTestId('header-history')).toHaveTextContent('History');
    await expect.element(screen.getByTestId('header-select')).toHaveTextContent('model-a');
    await expect.element(screen.getByTestId('header-search')).toHaveTextContent('draft query');

    await screen.rerender(
      <HeaderProvider>
        <HeaderRegistration active={false} />
        <HeaderProbe />
      </HeaderProvider>,
    );

    await expect.element(screen.getByTestId('header-title')).toHaveTextContent('Slab');
    await expect.element(screen.getByTestId('header-history')).toHaveTextContent('none');
    await expect.element(screen.getByTestId('header-select')).toHaveTextContent('none');
    await expect.element(screen.getByTestId('header-search')).toHaveTextContent('none');
  });

  it('uses the latest registration while preserving earlier entries', async () => {
    const screen = await render(
      <HeaderProvider>
        <SelectRegistration value="model-a" />
        <SelectRegistration value="model-b" />
        <HeaderProbe />
      </HeaderProvider>,
    );

    await expect.element(screen.getByTestId('header-select')).toHaveTextContent('model-b');
    await screen.unmount();
  });

  it('renders registered history as an actionable header button', async () => {
    onHistoryClick.mockClear();

    const screen = await render(
      <HeaderProvider>
        <HeaderRegistration active />
        <Header />
      </HeaderProvider>,
    );

    const historyButton = screen.getByTestId('header-history-control');
    await expect.element(historyButton).toHaveAttribute('aria-label', 'Open history');

    await historyButton.click();

    expect(onHistoryClick).toHaveBeenCalledTimes(1);
  });

  it('disables registered history actions while preserving the control', async () => {
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

    const screen = await render(
      <HeaderProvider>
        <DisabledHistoryRegistration />
        <Header />
      </HeaderProvider>,
    );

    const historyButton = screen.getByTestId('header-history-control');
    await expect.element(historyButton).toBeDisabled();

    // Raw DOM click on a disabled button: spec-mandated no-op (dispatches no
    // click event), and avoids the Locator click's actionability auto-wait.
    (historyButton.element() as HTMLElement).click();

    expect(onHistoryClick).not.toHaveBeenCalled();
  });

  it('removes the history button after registration cleanup', async () => {
    const screen = await render(
      <HeaderProvider>
        <HeaderRegistration active />
        <Header />
      </HeaderProvider>,
    );

    await expect.element(screen.getByTestId('header-history-control')).toBeInTheDocument();

    await screen.rerender(
      <HeaderProvider>
        <HeaderRegistration active={false} />
        <Header />
      </HeaderProvider>,
    );

    await expect.element(screen.getByTestId('header-history-control')).not.toBeInTheDocument();
  });

  it('does not republish equivalent history registrations', async () => {
    const onRender = vi.fn<(history: HeaderHistoryConfig | null) => void>();

    function StableHistoryRegistration({ value }: { value: string }) {
      useHeader({
        history: historyConfig,
      });

      return <span data-testid="stable-value">{value}</span>;
    }

    const screen = await render(
      <HeaderProvider>
        <StableHistoryRegistration value="first" />
        <HeaderRenderProbe onRender={onRender} />
      </HeaderProvider>,
    );

    await expect.element(screen.getByTestId('header-render-probe')).toHaveTextContent('History');
    expect(onRender).toHaveBeenCalledTimes(2);

    await screen.rerender(
      <HeaderProvider>
        <StableHistoryRegistration value="second" />
        <HeaderRenderProbe onRender={onRender} />
      </HeaderProvider>,
    );

    await expect.element(screen.getByTestId('stable-value')).toHaveTextContent('second');
    expect(onRender).toHaveBeenCalledTimes(2);
  });
});
