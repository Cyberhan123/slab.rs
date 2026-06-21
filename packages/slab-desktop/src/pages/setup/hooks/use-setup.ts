import { useCallback, useEffect, useRef, useState } from 'react';
import { useInterval } from '@mantine/hooks';
import { useNavigate } from 'react-router-dom';

import api, { getErrorMessage } from '@slab/api';
import { translateServerField, useTranslation } from '@slab/i18n';
import useDesktopPlatform from '@/hooks/use-desktop-platform';
import { queryClient } from '@/lib/query-client';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';

import {
  TASK_POLL_INTERVAL_MS,
  getProvisionProgressSummary,
  getProvisionProgressValue,
  getProvisionStageHint,
  getProvisionStageLabel,
  type ProvisionState,
  type RuntimePayloadMode,
  type SetupStatus,
  type TaskRecord,
} from '../const';

export interface SetupViewModel {
  setupStatus: SetupStatus | null;
  runtimePayloadMode: RuntimePayloadMode;
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
  const { t } = useTranslation();
  const navigate = useNavigate();
  const platform = useDesktopPlatform();
  const runtimePayloadMode: RuntimePayloadMode = platform === 'macos' ? 'bundled' : 'packaged';
  const autoStartedRef = useRef(false);
  const setupMountedRef = useRef(true);
  const provisionTaskIdRef = useRef<string | null>(null);

  usePageHeader({
    ...PAGE_HEADER_META.setup,
    title: t('pages.setup.header.title'),
    subtitle: t('pages.setup.header.subtitle'),
  });

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
    // Setup status drives navigation; failed probes should settle into the page
    // error state instead of stacking global retries while the server is booting.
    retry: false,
  });
  const provisionMutation = api.useMutation('post', '/v1/setup/provision', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });

  const status: SetupStatus | null = setupStatus ?? null;
  const runtimePayloadUsesBundledResources =
    runtimePayloadMode === 'bundled' || (status?.runtime_payload_installed ?? false);

  useEffect(() => {
    setupMountedRef.current = true;
    return () => {
      setupMountedRef.current = false;
      provisionTaskIdRef.current = null;
    };
  }, []);

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
      setProvisionError(getErrorMessage(error));
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
      setProvisionError(
        translateServerField(task.i18n, 'error_msg', task.error_msg, t) ||
          t('pages.setup.errors.failedBeforeFinish'),
      );
    },
    [markSetupInitialized, navigate, refetchSetupStatus, t],
  );

  useEffect(() => {
    provisionTaskIdRef.current = provisionTaskId;
  }, [provisionTaskId]);

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

  const pollProvisionTask = useCallback(async () => {
    const activeTaskId = provisionTaskId;
    if (!activeTaskId) {
      return;
    }

    try {
      const task = await getTaskMutation.mutateAsync({
        params: {
          path: { id: activeTaskId },
        },
      });

      if (!setupMountedRef.current || provisionTaskIdRef.current !== activeTaskId) {
        return;
      }

      await handleProvisionTask(task);
    } catch {
      // Ignore transient polling failures while the local host is still alive.
    }
  }, [getTaskMutation, handleProvisionTask, provisionTaskId]);
  const { start: startProvisionPoll, stop: stopProvisionPoll } = useInterval(() => {
    void pollProvisionTask();
  }, TASK_POLL_INTERVAL_MS);

  useEffect(() => {
    if (!provisionTaskId) {
      stopProvisionPoll();
      return undefined;
    }

    void pollProvisionTask();
    startProvisionPoll();
    return stopProvisionPoll;
  }, [pollProvisionTask, provisionTaskId, startProvisionPoll, stopProvisionPoll]);

  const handleRetry = useCallback(async () => {
    autoStartedRef.current = true;
    await startProvision();
  }, [startProvision]);
  const isCheckingSetupStatus =
    setupStatusLoading || (setupStatusFetching && provisionState === 'idle');

  return {
    setupStatus: status,
    runtimePayloadMode,
    isChecking: isCheckingSetupStatus,
    checkError: setupStatusError ? getErrorMessage(setupStatusError) : null,
    provisionState,
    provisionError,
    stageLabel: getProvisionStageLabel(
      provisionState,
      provisionTask,
      runtimePayloadUsesBundledResources,
      t,
    ),
    stageHint: getProvisionStageHint(
      provisionState,
      provisionTask,
      runtimePayloadUsesBundledResources,
      t,
    ),
    progressPercent: getProvisionProgressValue(provisionState, provisionTask),
    progressSummary: getProvisionProgressSummary(
      provisionState,
      provisionTask,
      runtimePayloadUsesBundledResources,
      t,
    ),
    canRetry: provisionState === 'failed',
    handleRetry,
  };
}
