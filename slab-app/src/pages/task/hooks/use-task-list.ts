import { useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';

import api from '@/lib/api';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import { PAGE_SIZE, type Task, type TaskResult } from '../const';
import { getSparklineWeight, getTaskDurationMs, isSettledStatus } from '../utils';

export function useTaskList() {
  usePageHeader(PAGE_HEADER_META.task);

  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [taskResult, setTaskResult] = useState<TaskResult | null>(null);
  const [page, setPage] = useState(1);

  const {
    data: tasks,
    error: tasksError,
    isLoading: tasksLoading,
    refetch: refetchTasks,
  } = api.useQuery('get', '/v1/tasks', {
    params: {
      path: { type: null },
    },
  });

  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');
  const getTaskResultMutation = api.useMutation('get', '/v1/tasks/{id}/result');
  const cancelTaskMutation = api.useMutation('post', '/v1/tasks/{id}/cancel');
  const restartTaskMutation = api.useMutation('post', '/v1/tasks/{id}/restart');

  const allTasks = useMemo<Task[]>(() => (Array.isArray(tasks) ? tasks : []), [tasks]);

  const metrics = useMemo(
    () => ({
      total: allTasks.length,
      running: allTasks.filter((task) => task.status === 'running').length,
      queued: allTasks.filter((task) => task.status === 'pending').length,
      failed: allTasks.filter((task) => task.status === 'failed').length,
      succeeded: allTasks.filter((task) => task.status === 'succeeded').length,
    }),
    [allTasks],
  );

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

    const totalDuration = settledTasks.reduce((sum, task) => {
      return sum + getTaskDurationMs(task);
    }, 0);

    return totalDuration / settledTasks.length;
  }, [settledTasks]);

  const successSparkline = useMemo(() => {
    const recentTasks = [...allTasks]
      .sort((left, right) => Date.parse(left.updated_at) - Date.parse(right.updated_at))
      .slice(-7);

    if (recentTasks.length === 0) {
      return [0.32, 0.48, 0.44, 0.66, 0.82, 0.72, 0.77];
    }

    return recentTasks.map((task) => getSparklineWeight(task.status));
  }, [allTasks]);

  const durationSparkline = useMemo(() => {
    const samples = [...settledTasks]
      .sort((left, right) => Date.parse(left.updated_at) - Date.parse(right.updated_at))
      .slice(-5)
      .map((task) => getTaskDurationMs(task));

    if (samples.length === 0) {
      return [0.18, 0.28, 0.24, 0.6, 0.44];
    }

    const maxSample = Math.max(...samples, 1);

    return samples.map((sample) => Math.max(sample / maxSample, 0.16));
  }, [settledTasks]);

  const totalPages = Math.max(1, Math.ceil(allTasks.length / PAGE_SIZE));
  const currentPage = Math.min(page, totalPages);

  const paginatedTasks = useMemo(() => {
    const startIndex = (currentPage - 1) * PAGE_SIZE;
    return allTasks.slice(startIndex, startIndex + PAGE_SIZE);
  }, [allTasks, currentPage]);

  const paginationLabel = useMemo(() => {
    if (allTasks.length === 0) {
      return 'Showing 0 to 0 of 0 entries';
    }

    const start = (currentPage - 1) * PAGE_SIZE + 1;
    const end = Math.min(currentPage * PAGE_SIZE, allTasks.length);

    return `Showing ${start} to ${end} of ${allTasks.length} entries`;
  }, [allTasks.length, currentPage]);

  const fetchTaskResult = async (id: string) => {
    try {
      const data = await getTaskResultMutation.mutateAsync({
        params: {
          path: { id },
        },
      });

      setTaskResult(data);
    } catch (err) {
      toast.error(`Failed to fetch task result: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  };

  const fetchTaskDetail = async (id: string) => {
    try {
      setTaskResult(null);
      const data = await getTaskMutation.mutateAsync({
        params: {
          path: { id },
        },
      });

      setSelectedTask(data);

      if (data.status === 'succeeded') {
        await fetchTaskResult(id);
      }
    } catch {
      toast.error('Failed to fetch task details');
    }
  };

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
      toast.error(`Failed to cancel task: ${err instanceof Error ? err.message : 'Unknown error'}`);
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
      toast.error(`Failed to restart task: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  };

  useEffect(() => {
    if (page > totalPages) {
      setPage(totalPages);
    }
  }, [page, totalPages]);

  useEffect(() => {
    const hasRunningTasks = allTasks.some((task) => task.status === 'running');
    if (!hasRunningTasks) return;

    const interval = setInterval(() => {
      void refetchTasks();
    }, 3000);

    return () => clearInterval(interval);
  }, [allTasks, refetchTasks]);

  useEffect(() => {
    if (!selectedTask || selectedTask.status !== 'running') return;

    const interval = setInterval(() => {
      void fetchTaskDetail(selectedTask.id);
    }, 3000);

    return () => clearInterval(interval);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedTask?.status, selectedTask?.id]);

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
