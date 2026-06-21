import { useCallback, useEffect, useMemo, useState } from 'react';
import { useInterval } from '@mantine/hooks';
import { clamp, countBy, sortBy, sumBy } from 'lodash-es';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import api from '@slab/api';
import { getErrorDescription } from '@/lib/error-description';
import { PAGE_SIZE, TASK_LIST_POLL_INTERVAL_MS, type Task, type TaskResult } from '../const';
import { getSparklineWeight, getTaskDurationMs, isMediaTaskType, isSettledStatus } from '../utils';

export function useTaskList() {
  const { t } = useTranslation();

  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [taskResult, setTaskResult] = useState<TaskResult | null>(null);
  const [page, setPage] = useState(1);

  const {
    data: tasks,
    error: tasksError,
    isLoading: tasksLoading,
    refetch: refetchTasks,
  } = api.useQuery('get', '/v1/tasks');

  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const getTaskResultMutation = api.useMutation('get', '/v1/tasks/{id}/result', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const cancelTaskMutation = api.useMutation('post', '/v1/tasks/{id}/cancel', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const restartTaskMutation = api.useMutation('post', '/v1/tasks/{id}/restart', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });

  const allTasks = useMemo<Task[]>(() => (Array.isArray(tasks) ? tasks : []), [tasks]);

  const metrics = useMemo(() => {
    const byStatus = countBy(allTasks, 'status');
    return {
      total: allTasks.length,
      running: byStatus.running ?? 0,
      queued: byStatus.pending ?? 0,
      failed: byStatus.failed ?? 0,
      succeeded: byStatus.succeeded ?? 0,
    };
  }, [allTasks]);

  const settledTasks = useMemo(
    () => allTasks.filter((task) => isSettledStatus(task.status)),
    [allTasks],
  );

  const successRate = useMemo(() => {
    if (settledTasks.length === 0) return 0;
    return (metrics.succeeded / settledTasks.length) * 100;
  }, [metrics.succeeded, settledTasks.length]);

  const activeTaskCount = metrics.running + metrics.queued;
  const activeShare = metrics.total > 0 ? (activeTaskCount / metrics.total) * 100 : 0;

  const averageTurnaroundMs = useMemo(() => {
    if (settledTasks.length === 0) return 0;

    const totalDuration = sumBy(settledTasks, getTaskDurationMs);

    return totalDuration / settledTasks.length;
  }, [settledTasks]);

  const successSparkline = useMemo(() => {
    const recentTasks = sortBy(allTasks, (task) => Date.parse(task.updated_at)).slice(-7);

    if (recentTasks.length === 0) {
      return [0.32, 0.48, 0.44, 0.66, 0.82, 0.72, 0.77];
    }

    return recentTasks.map((task) => getSparklineWeight(task.status));
  }, [allTasks]);

  const durationSparkline = useMemo(() => {
    const samples = sortBy(settledTasks, (task) => Date.parse(task.updated_at))
      .slice(-5)
      .map((task) => getTaskDurationMs(task));

    if (samples.length === 0) {
      return [0.18, 0.28, 0.24, 0.6, 0.44];
    }

    const maxSample = Math.max(...samples, 1);

    return samples.map((sample) => clamp(sample / maxSample, 0.16, Number.POSITIVE_INFINITY));
  }, [settledTasks]);

  const totalPages = Math.max(1, Math.ceil(allTasks.length / PAGE_SIZE));
  const currentPage = clamp(page, 1, totalPages);

  const paginatedTasks = useMemo(() => {
    const startIndex = (currentPage - 1) * PAGE_SIZE;
    return allTasks.slice(startIndex, startIndex + PAGE_SIZE);
  }, [allTasks, currentPage]);

  const paginationLabel = useMemo(() => {
    if (allTasks.length === 0) {
      return t('pages.task.table.pagination.empty');
    }

    const start = (currentPage - 1) * PAGE_SIZE + 1;
    const end = clamp(currentPage * PAGE_SIZE, 0, allTasks.length);

    return t('pages.task.table.pagination.summary', {
      start,
      end,
      total: allTasks.length,
    });
  }, [allTasks.length, currentPage, t]);

  const fetchTaskResult = useCallback(async (id: string) => {
    try {
      const data = await getTaskResultMutation.mutateAsync({
        params: {
          path: { id },
        },
      });

      setTaskResult(data);
    } catch (err) {
      toast.error(
        t('pages.task.toast.fetchTaskResultFailed', {
          message: getErrorDescription(err, t('pages.task.toast.unknownError')),
        }),
      );
    }
  }, [getTaskResultMutation, t]);

  const fetchTaskDetail = useCallback(async (id: string) => {
    try {
      setTaskResult(null);
      const data = await getTaskMutation.mutateAsync({
        params: {
          path: { id },
        },
      });

      setSelectedTask(data);

      if (data.status === 'succeeded' && !isMediaTaskType(data.task_type)) {
        await fetchTaskResult(id);
      }
    } catch {
      toast.error(t('pages.task.toast.fetchTaskDetailsFailed'));
    }
  }, [fetchTaskResult, getTaskMutation, t]);

  const cancelTask = async (id: string) => {
    try {
      await cancelTaskMutation.mutateAsync({
        params: {
          path: { id },
        },
      });

      void refetchTasks();
      if (selectedTask?.id === id) {
        await fetchTaskDetail(id);
      }
    } catch (err) {
      toast.error(
        t('pages.task.toast.cancelTaskFailed', {
          message: getErrorDescription(err, t('pages.task.toast.unknownError')),
        }),
      );
    }
  };

  const restartTask = async (id: string) => {
    try {
      await restartTaskMutation.mutateAsync({
        params: {
          path: { id },
        },
      });

      void refetchTasks();
      if (selectedTask?.id === id) {
        await fetchTaskDetail(id);
      }
    } catch (err) {
      toast.error(
        t('pages.task.toast.restartTaskFailed', {
          message: getErrorDescription(err, t('pages.task.toast.unknownError')),
        }),
      );
    }
  };

  useEffect(() => {
    if (page > totalPages) {
      setPage(totalPages);
    }
  }, [page, totalPages]);

  const hasRunningTasks = allTasks.some((task) => task.status === 'running');
  const { start: startTaskPoll, stop: stopTaskPoll } = useInterval(() => {
    void refetchTasks();
  }, TASK_LIST_POLL_INTERVAL_MS);
  const { start: startSelectedTaskPoll, stop: stopSelectedTaskPoll } = useInterval(() => {
    if (selectedTask) {
      void fetchTaskDetail(selectedTask.id);
    }
  }, TASK_LIST_POLL_INTERVAL_MS);

  useEffect(() => {
    if (hasRunningTasks) {
      startTaskPoll();
      return stopTaskPoll;
    }

    stopTaskPoll();
    return undefined;
  }, [hasRunningTasks, startTaskPoll, stopTaskPoll]);

  useEffect(() => {
    if (selectedTask?.status === 'running') {
      startSelectedTaskPoll();
      return stopSelectedTaskPoll;
    }

    stopSelectedTaskPoll();
    return undefined;
  }, [selectedTask?.status, startSelectedTaskPoll, stopSelectedTaskPoll]);

  return {
    allTasks,
    metrics,
    settledTasks,
    successRate,
    activeTaskCount,
    activeShare,
    averageTurnaroundMs,
    successSparkline,
    durationSparkline,
    totalPages,
    currentPage,
    paginatedTasks,
    paginationLabel,
    selectedTask,
    setSelectedTask,
    taskResult,
    tasksError,
    tasksLoading,
    cancelTaskMutation,
    restartTaskMutation,
    fetchTaskDetail,
    cancelTask,
    restartTask,
    setPage,
  };
}
