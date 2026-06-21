import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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

  it('shows Checking only during the first load', () => {
    const { rerender } = render(<BackendStatus />);
    expect(screen.getByText('Checking...')).toBeInTheDocument();

    healthResult = {
      ...healthResult,
      data: { status: 'ok' },
      dataUpdatedAt: 1,
      isLoading: false,
    };
    rerender(<BackendStatus />);

    expect(screen.getByText('Online')).toBeInTheDocument();
    expect(screen.queryByText('Checking...')).not.toBeInTheDocument();
  });

  it('requires three consecutive failed health probes before showing Offline', async () => {
    const { rerender } = render(<BackendStatus />);

    healthResult = {
      ...healthResult,
      data: { status: 'ok' },
      dataUpdatedAt: 1,
      isLoading: false,
    };
    rerender(<BackendStatus />);
    await waitFor(() => expect(screen.getByText('Online')).toBeInTheDocument());

    healthResult = {
      ...healthResult,
      error: new Error('offline'),
      errorUpdatedAt: 2,
    };
    rerender(<BackendStatus />);
    await waitFor(() => expect(screen.getByText('Online')).toBeInTheDocument());

    healthResult = {
      ...healthResult,
      error: new Error('offline'),
      errorUpdatedAt: 3,
    };
    rerender(<BackendStatus />);
    await waitFor(() => expect(screen.getByText('Online')).toBeInTheDocument());

    healthResult = {
      ...healthResult,
      error: new Error('offline'),
      errorUpdatedAt: 4,
    };
    rerender(<BackendStatus />);

    await waitFor(() => expect(screen.getByRole('button', { name: 'Offline' })).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: 'Offline' }));
    expect(refetch).toHaveBeenCalledTimes(1);
  });
});
