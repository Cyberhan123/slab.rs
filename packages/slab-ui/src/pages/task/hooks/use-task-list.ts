import { useCallback, useEffect, useMemo, useState } from 'react';
import { useInterval } from '@mantine/hooks';
import { clamp } from 'lodash-es';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import api from '@slab/api';
import { getErrorDescription } from '@slab/core/api/error-description';
import { TASK_LIST_POLL_INTERVAL_MS, type Task, type TaskResult } from '../const';
import { isMediaTaskType } from '../utils';
import {
  computeAverageTurnaroundMs,
  computeDurationSparkline,
  computePaginationRange,
  computeSuccessRate,
  computeSuccessSparkline,
  computeTaskMetrics,
  computeTotalPages,
  paginateTasks,
  selectSettledTasks,
} from '../lib/task-list-metrics';

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

  const metrics = useMemo(() => computeTaskMetrics(allTasks), [allTasks]);

  const settledTasks = useMemo(() => selectSettledTasks(allTasks), [allTasks]);

  const successRate = useMemo(
    () => computeSuccessRate(settledTasks.length, metrics.succeeded),
    [metrics.succeeded, settledTasks.length],
  );

  const activeTaskCount = metrics.running + metrics.queued;
  const activeShare = metrics.total > 0 ? (activeTaskCount / metrics.total) * 100 : 0;

  const averageTurnaroundMs = useMemo(
    () => computeAverageTurnaroundMs(settledTasks),
    [settledTasks],
  );

  const successSparkline = useMemo(() => computeSuccessSparkline(allTasks), [allTasks]);

  const durationSparkline = useMemo(
    () => computeDurationSparkline(settledTasks),
    [settledTasks],
  );

  const totalPages = computeTotalPages(allTasks.length);
  const currentPage = clamp(page, 1, totalPages);

  const paginatedTasks = useMemo(
    () => paginateTasks(allTasks, currentPage),
    [allTasks, currentPage],
  );

  const paginationLabel = useMemo(() => {
    const range = computePaginationRange(allTasks.length, currentPage);
    if (!range) {
      return t('pages.task.table.pagination.empty');
    }

    return t('pages.task.table.pagination.summary', {
      start: range.start,
      end: range.end,
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
          message: getErrorDescription(err, t('common.toasts.unknownError')),
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
          message: getErrorDescription(err, t('common.toasts.unknownError')),
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
          message: getErrorDescription(err, t('common.toasts.unknownError')),
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
