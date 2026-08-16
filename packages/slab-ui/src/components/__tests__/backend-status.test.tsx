import { render } from 'vitest-browser-react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const apiMock = vi.hoisted(() => ({
  useQuery: vi.fn<() => unknown>(),
}));

vi.mock('@slab/api', () => ({
  default: apiMock,
}));

import { BackendStatus } from '../backend-status';

describe('BackendStatus', () => {
  const refetch = vi.fn<() => void>();
  let healthResult: {
    data: unknown;
    dataUpdatedAt: number;
    error: unknown;
    errorUpdatedAt: number;
    isLoading: boolean;
    refetch: () => void;
  };

  beforeEach(() => {
    refetch.mockClear();
    healthResult = {
      data: undefined,
      dataUpdatedAt: 0,
      error: null,
      errorUpdatedAt: 0,
      isLoading: true,
      refetch,
    };
    apiMock.useQuery.mockImplementation(() => healthResult);
  });

  it('shows Checking only during the first load', async () => {
    const screen = await render(<BackendStatus />);
    await expect.element(screen.getByText('Checking...')).toBeInTheDocument();

    healthResult = {
      ...healthResult,
      data: { status: 'ok' },
      dataUpdatedAt: 1,
      isLoading: false,
    };
    await screen.rerender(<BackendStatus />);

    await expect.element(screen.getByText('Online')).toBeInTheDocument();
    expect(screen.getByText('Checking...').query()).toBeNull();
  });

  it('requires three consecutive failed health probes before showing Offline', async () => {
    const screen = await render(<BackendStatus />);

    healthResult = {
      ...healthResult,
      data: { status: 'ok' },
      dataUpdatedAt: 1,
      isLoading: false,
    };
    await screen.rerender(<BackendStatus />);
    await expect.element(screen.getByText('Online')).toBeInTheDocument();

    healthResult = {
      ...healthResult,
      error: new Error('offline'),
      errorUpdatedAt: 2,
    };
    await screen.rerender(<BackendStatus />);
    await expect.element(screen.getByText('Online')).toBeInTheDocument();

    healthResult = {
      ...healthResult,
      error: new Error('offline'),
      errorUpdatedAt: 3,
    };
    await screen.rerender(<BackendStatus />);
    await expect.element(screen.getByText('Online')).toBeInTheDocument();

    healthResult = {
      ...healthResult,
      error: new Error('offline'),
      errorUpdatedAt: 4,
    };
    await screen.rerender(<BackendStatus />);

    await expect.element(screen.getByRole('button', { name: 'Offline' })).toBeInTheDocument();
    await screen.getByRole('button', { name: 'Offline' }).click();
    expect(refetch).toHaveBeenCalledTimes(1);
  });
});
