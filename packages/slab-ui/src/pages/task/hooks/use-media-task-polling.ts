import { useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';

import api, { getErrorMessage } from '@slab/api';
import type { components } from '@slab/api/v1';

type TaskResponse = components['schemas']['TaskResponse'];

type UseMediaTaskPollingOptions = {
  enabled: boolean;
  intervalMs: number;
  maxErrorIntervalMs?: number;
  pollingErrorToastId: string;
  taskId: string | null;
  toPollingErrorMessage: (message: string) => string;
};

export function useMediaTaskPolling({
  enabled,
  intervalMs,
  maxErrorIntervalMs = 10_000,
  pollingErrorToastId,
  taskId,
  toPollingErrorMessage,
}: UseMediaTaskPollingOptions) {
  const [pollIntervalMs, setPollIntervalMs] = useState(intervalMs);
  const [consecutiveErrors, setConsecutiveErrors] = useState(0);
  const consecutiveErrorsRef = useRef(0);
  const handledErrorUpdatedAtRef = useRef(0);
  const queryEnabled = enabled && Boolean(taskId);
  const {
    data,
    dataUpdatedAt,
    error,
    errorUpdatedAt,
  } = api.useQuery(
    'get',
    '/v1/tasks/{id}',
    {
      params: {
        path: {
          id: taskId ?? '',
        },
      },
    },
    {
      enabled: queryEnabled,
      refetchInterval: queryEnabled ? pollIntervalMs : false,
      refetchIntervalInBackground: true,
      // This hook owns media polling backoff and toast dedupe; React Query retry
      // would stack extra probes on top of the task interval.
      retry: false,
    },
  ) as {
    data: TaskResponse | undefined;
    dataUpdatedAt: number;
    error: unknown;
    errorUpdatedAt: number;
  };

  useEffect(() => {
    setPollIntervalMs(intervalMs);
    setConsecutiveErrors(0);
    consecutiveErrorsRef.current = 0;
    handledErrorUpdatedAtRef.current = 0;
  }, [intervalMs, taskId]);

  useEffect(() => {
    if (!queryEnabled || dataUpdatedAt === 0) {
      return;
    }

    setPollIntervalMs(intervalMs);
    setConsecutiveErrors(0);
    consecutiveErrorsRef.current = 0;
  }, [dataUpdatedAt, intervalMs, queryEnabled]);

  useEffect(() => {
    if (!queryEnabled || !error || errorUpdatedAt === 0) {
      return;
    }
    if (handledErrorUpdatedAtRef.current === errorUpdatedAt) {
      return;
    }

    handledErrorUpdatedAtRef.current = errorUpdatedAt;
    const nextConsecutiveErrors = consecutiveErrorsRef.current + 1;
    consecutiveErrorsRef.current = nextConsecutiveErrors;
    setConsecutiveErrors(nextConsecutiveErrors);
    setPollIntervalMs(Math.min(maxErrorIntervalMs, intervalMs * 2 ** nextConsecutiveErrors));
    toast.error(toPollingErrorMessage(getErrorMessage(error)), {
      id: pollingErrorToastId,
    });
  }, [
    error,
    errorUpdatedAt,
    intervalMs,
    maxErrorIntervalMs,
    pollingErrorToastId,
    queryEnabled,
    toPollingErrorMessage,
  ]);

  return {
    consecutiveErrors,
    taskStatus: data,
    taskStatusUpdatedAt: dataUpdatedAt,
  };
}
