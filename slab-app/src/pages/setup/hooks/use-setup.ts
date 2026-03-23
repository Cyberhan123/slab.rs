import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import api from '@/lib/api';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';

import {
  PROGRESS_MAX_SIMULATED,
  PROGRESS_STEP,
  TASK_POLL_INTERVAL_MS,
  type DownloadState,
  type SetupStatus,
  type TaskRecord,
} from '../const';

function toErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export interface SetupViewModel {
  status: SetupStatus | null;
  isChecking: boolean;
  checkError: string | null;
  ffmpegDownload: DownloadState;
  ffmpegError: string | null;
  ffmpegProgress: number;
  ffmpegReady: boolean;
  allBackendsUnavailable: boolean;
  completing: boolean;
  saveError: string | null;
  handleDownloadFfmpeg: () => Promise<void>;
  handleComplete: () => Promise<void>;
}

export function useSetup(): SetupViewModel {
  const navigate = useNavigate();
  const pollIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  usePageHeader(PAGE_HEADER_META.setup);

  const [ffmpegDownload, setFfmpegDownload] = useState<DownloadState>('idle');
  const [ffmpegTaskId, setFfmpegTaskId] = useState<string | null>(null);
  const [ffmpegError, setFfmpegError] = useState<string | null>(null);
  const [ffmpegProgress, setFfmpegProgress] = useState(0);
  const [saveError, setSaveError] = useState<string | null>(null);

  const {
    data: setupStatus,
    error: setupStatusError,
    isLoading: setupStatusLoading,
    refetch: refetchSetupStatus,
  } = api.useQuery('get', '/v1/setup/status');
  const downloadFfmpegMutation = api.useMutation('post', '/v1/setup/ffmpeg/download');
  const completeSetupMutation = api.useMutation('post', '/v1/setup/complete');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const status = setupStatus ?? null;

  useEffect(() => {
    if (status?.initialized) {
      navigate('/', { replace: true });
    }
  }, [navigate, status?.initialized]);

  const handleFfmpegDone = useCallback(
    async (task: TaskRecord) => {
      if (task.status === 'succeeded') {
        setFfmpegDownload('done');
        setFfmpegProgress(100);
        setFfmpegError(null);
        setFfmpegTaskId(null);
        await refetchSetupStatus();
        return;
      }

      setFfmpegDownload('error');
      setFfmpegError(task.error_msg ?? 'Download failed');
      setFfmpegProgress(0);
      setFfmpegTaskId(null);
    },
    [refetchSetupStatus],
  );

  useEffect(() => {
    if (!ffmpegTaskId) {
      return;
    }

    const poll = async () => {
      try {
        const task = await getTaskMutation.mutateAsync({
          params: {
            path: { id: ffmpegTaskId },
          },
        });

        if (task.status === 'succeeded' || task.status === 'failed') {
          if (pollIntervalRef.current) {
            clearInterval(pollIntervalRef.current);
          }
          await handleFfmpegDone(task);
        }
      } catch {
        // Ignore transient polling failures and keep monitoring the task.
      }
    };

    void poll();
    pollIntervalRef.current = setInterval(() => {
      void poll();
    }, TASK_POLL_INTERVAL_MS);

    return () => {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
      }
    };
  }, [ffmpegTaskId, getTaskMutation, handleFfmpegDone]);

  useEffect(() => {
    if (ffmpegDownload !== 'downloading') {
      return;
    }

    const timer = setInterval(() => {
      setFfmpegProgress((current) =>
        Math.min(current + PROGRESS_STEP, PROGRESS_MAX_SIMULATED),
      );
    }, 800);

    return () => {
      clearInterval(timer);
    };
  }, [ffmpegDownload]);

  const handleDownloadFfmpeg = useCallback(async () => {
    setSaveError(null);
    setFfmpegDownload('downloading');
    setFfmpegError(null);
    setFfmpegProgress(5);

    try {
      const operation = await downloadFfmpegMutation.mutateAsync({});
      setFfmpegTaskId(operation.operation_id);
    } catch (error) {
      setFfmpegDownload('error');
      setFfmpegError(toErrorMessage(error));
      setFfmpegProgress(0);
    }
  }, [downloadFfmpegMutation]);

  const handleComplete = useCallback(async () => {
    setSaveError(null);

    try {
      await completeSetupMutation.mutateAsync({
        body: { initialized: true },
      });
      navigate('/', { replace: true });
    } catch (error) {
      setSaveError(toErrorMessage(error));
    }
  }, [completeSetupMutation, navigate]);

  return {
    status,
    isChecking: setupStatusLoading,
    checkError: setupStatusError ? toErrorMessage(setupStatusError) : null,
    ffmpegDownload,
    ffmpegError,
    ffmpegProgress,
    ffmpegReady: Boolean(status?.ffmpeg.installed) || ffmpegDownload === 'done',
    allBackendsUnavailable: Boolean(
      status && status.backends.every((backend) => !backend.installed),
    ),
    completing: completeSetupMutation.isPending,
    saveError,
    handleDownloadFfmpeg,
    handleComplete,
  };
}
