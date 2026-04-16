import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import api, { queryClient } from '@/lib/api';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';

import {
  TASK_POLL_INTERVAL_MS,
  getProvisionProgressSummary,
  getProvisionProgressValue,
  getProvisionStageHint,
  getProvisionStageLabel,
  type ProvisionState,
  type SetupStatus,
  type TaskRecord,
} from '../const';

function toErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function isTerminalTaskStatus(status: TaskRecord['status']) {
  return status === 'succeeded'
    || status === 'failed'
    || status === 'cancelled'
    || status === 'interrupted';
}

export interface SetupViewModel {
  setupStatus: SetupStatus | null;
  isChecking: boolean;
  checkError: string | null;
  provisionState: ProvisionState;
  provisionError: string | null;
  stageLabel: string;
  stageHint: string;
  progressPercent: number;
  progressSummary: string;
  canRetry: boolean;
  handleRetry: () => Promise<void>;
}

export function useSetup(): SetupViewModel {
  const navigate = useNavigate();
  const autoStartedRef = useRef(false);
  const pollIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  usePageHeader(PAGE_HEADER_META.setup);

  const [provisionState, setProvisionState] = useState<ProvisionState>('idle');
  const [provisionTaskId, setProvisionTaskId] = useState<string | null>(null);
  const [provisionTask, setProvisionTask] = useState<TaskRecord | null>(null);
  const [provisionError, setProvisionError] = useState<string | null>(null);

  const {
    data: setupStatus,
    error: setupStatusError,
    isLoading: setupStatusLoading,
    isFetching: setupStatusFetching,
    refetch: refetchSetupStatus,
  } = api.useQuery('get', '/v1/setup/status', undefined, {
    staleTime: 0,
    refetchOnMount: 'always',
    refetchOnReconnect: true,
    refetchOnWindowFocus: true,
    retry: false,
  });
  const provisionMutation = api.useMutation('post', '/v1/setup/provision');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const status: SetupStatus | null = setupStatus ?? null;

  const markSetupInitialized = useCallback(() => {
    queryClient.setQueriesData(
      {
        predicate: (query) => JSON.stringify(query.queryKey).includes('/v1/setup/status'),
      },
      (current) => {
        if (!current || typeof current !== 'object') {
          return current;
        }

        return {
          ...current,
          initialized: true,
        };
      },
    );
  }, []);

  useEffect(() => {
    if (setupStatusFetching) {
      return;
    }

    if (status?.initialized) {
      markSetupInitialized();
      navigate('/', { replace: true });
    }
  }, [markSetupInitialized, navigate, setupStatusFetching, status?.initialized]);

  const startProvision = useCallback(async () => {
    setProvisionState('starting');
    setProvisionError(null);
    setProvisionTask(null);
    setProvisionTaskId(null);

    try {
      const operation = await provisionMutation.mutateAsync({});
      setProvisionTaskId(operation.operation_id);
    } catch (error) {
      setProvisionState('failed');
      setProvisionError(toErrorMessage(error));
    }
  }, [provisionMutation]);

  const handleProvisionTask = useCallback(
    async (task: TaskRecord) => {
      setProvisionTask(task);

      if (task.status === 'pending') {
        setProvisionState('starting');
        return;
      }

      if (task.status === 'running') {
        setProvisionState('running');
        setProvisionError(null);
        return;
      }

      if (task.status === 'succeeded') {
        setProvisionState('succeeded');
        setProvisionError(null);
        setProvisionTaskId(null);
        markSetupInitialized();

        try {
          await refetchSetupStatus();
        } catch {
          // Keep going; the cache has already been marked initialized.
        }

        navigate('/', { replace: true });
        return;
      }

      setProvisionState('failed');
      setProvisionTaskId(null);
      setProvisionError(task.error_msg ?? 'Setup failed before provisioning could finish.');
    },
    [markSetupInitialized, navigate, refetchSetupStatus],
  );

  useEffect(() => {
    if (autoStartedRef.current) {
      return;
    }

    if (
      setupStatusLoading
      || setupStatusFetching
      || setupStatusError
      || !status
      || status.initialized
    ) {
      return;
    }

    autoStartedRef.current = true;
    void startProvision();
  }, [setupStatusError, setupStatusFetching, setupStatusLoading, startProvision, status]);

  useEffect(() => {
    if (!provisionTaskId) {
      return;
    }

    let disposed = false;

    const poll = async () => {
      try {
        const task = await getTaskMutation.mutateAsync({
          params: {
            path: { id: provisionTaskId },
          },
        });

        if (disposed) {
          return;
        }

        if (isTerminalTaskStatus(task.status) && pollIntervalRef.current) {
          clearInterval(pollIntervalRef.current);
          pollIntervalRef.current = null;
        }

        await handleProvisionTask(task);
      } catch {
        // Ignore transient polling failures while the local host is still alive.
      }
    };

    void poll();
    pollIntervalRef.current = setInterval(() => {
      void poll();
    }, TASK_POLL_INTERVAL_MS);

    return () => {
      disposed = true;
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
    };
  }, [getTaskMutation, handleProvisionTask, provisionTaskId]);

  const handleRetry = useCallback(async () => {
    autoStartedRef.current = true;
    await startProvision();
  }, [startProvision]);
  const isCheckingSetupStatus =
    setupStatusLoading || (setupStatusFetching && provisionState === 'idle');

  return {
    setupStatus: status,
    isChecking: isCheckingSetupStatus,
    checkError: setupStatusError ? toErrorMessage(setupStatusError) : null,
    provisionState,
    provisionError,
    stageLabel: getProvisionStageLabel(
      provisionState,
      provisionTask,
      status?.runtime_payload_installed ?? false,
    ),
    stageHint: getProvisionStageHint(
      provisionState,
      provisionTask,
      status?.runtime_payload_installed ?? false,
    ),
    progressPercent: getProvisionProgressValue(provisionState, provisionTask),
    progressSummary: getProvisionProgressSummary(
      provisionState,
      provisionTask,
      status?.runtime_payload_installed ?? false,
    ),
    canRetry: provisionState === 'failed',
    handleRetry,
  };
}
