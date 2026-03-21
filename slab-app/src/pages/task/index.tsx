import { useEffect, useMemo, useState } from 'react';
import {
  ListChecks,
  Loader2,
  PlayCircle,
  RefreshCw,
  Timer,
  TriangleAlert,
} from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Spinner } from '@/components/ui/spinner';
import {
  MetricCard,
  PillFilterBar,
  SoftPanel,
  StageEmptyState,
  StatusPill,
} from '@/components/ui/workspace';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import api from '@/lib/api';
import { toast } from 'sonner';

interface Task {
  id: string;
  status: string;
  task_type: string;
  error_msg?: string | null;
  created_at: string;
  updated_at: string;
}

interface TaskResult {
  [key: string]: any;
}

export default function Task() {
  usePageHeader(PAGE_HEADER_META.task);

  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [taskResult, setTaskResult] = useState<TaskResult | null>(null);
  const [taskType, setTaskType] = useState<string>('all');

  const {
    data: tasks,
    error: tasksError,
    isLoading: tasksLoading,
    isRefetching,
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
  const filteredTasks = useMemo(() => {
    if (taskType === 'all') return allTasks;
    return allTasks.filter((task) => task.task_type === taskType);
  }, [allTasks, taskType]);

  const metrics = useMemo(
    () => ({
      total: allTasks.length,
      running: allTasks.filter((task) => task.status === 'running').length,
      failed: allTasks.filter((task) => task.status === 'failed').length,
      succeeded: allTasks.filter((task) => task.status === 'succeeded').length,
    }),
    [allTasks],
  );

  useEffect(() => {
    const hasRunningTasks = filteredTasks.some((task) => task.status === 'running');
    if (!hasRunningTasks) return;

    const interval = setInterval(() => {
      void refetchTasks();
    }, 3000);

    return () => clearInterval(interval);
  }, [filteredTasks, refetchTasks]);

  useEffect(() => {
    if (!selectedTask || selectedTask.status !== 'running') return;

    const interval = setInterval(() => {
      void fetchTaskDetail(selectedTask.id);
    }, 3000);

    return () => clearInterval(interval);
  }, [selectedTask?.status, selectedTask?.id]);

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

  return (
    <div className="h-full w-full overflow-auto">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-5 px-1 pb-8">
        <SoftPanel className="workspace-halo space-y-5 overflow-hidden rounded-[30px] border border-border/70">
          <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
            <div className="space-y-2">
              <Badge variant="chip">Task operations</Badge>
              <h1 className="text-2xl font-semibold tracking-tight md:text-3xl">
                Task Workbench
              </h1>
              <p className="max-w-3xl text-sm leading-6 text-muted-foreground">
                Monitor long-running AI jobs, inspect task details, and recover failed or cancelled tasks from one console.
              </p>
            </div>
            <Button
              variant="pill"
              size="pill"
              disabled={isRefetching}
              onClick={() => void refetchTasks()}
            >
              {isRefetching ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <RefreshCw className="mr-2 h-4 w-4" />
              )}
              Refresh
            </Button>
          </div>

          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <MetricCard label="Total Tasks" value={metrics.total} hint="Current list size" icon={ListChecks} />
            <MetricCard label="Running" value={metrics.running} hint="Actively processing" icon={Timer} />
            <MetricCard label="Succeeded" value={metrics.succeeded} hint="Finished successfully" icon={PlayCircle} />
            <MetricCard label="Failed" value={metrics.failed} hint="Needs attention" icon={TriangleAlert} />
          </div>
        </SoftPanel>

        <PillFilterBar>
          <Select value={taskType} onValueChange={setTaskType}>
            <SelectTrigger variant="pill" size="pill" className="min-w-[210px]">
              <SelectValue placeholder="Task type" />
            </SelectTrigger>
            <SelectContent variant="pill">
              <SelectItem value="all">All types</SelectItem>
              <SelectItem value="transcription">Audio transcription</SelectItem>
              <SelectItem value="image_generation">Image generation</SelectItem>
              <SelectItem value="model_download">Model download</SelectItem>
            </SelectContent>
          </Select>
          <Badge variant="counter">{filteredTasks.length} visible</Badge>
          {isRefetching ? <StatusPill status="info">Syncing...</StatusPill> : null}
        </PillFilterBar>

        {tasksError ? (
          <Alert variant="destructive">
            <AlertTitle>Error</AlertTitle>
            <AlertDescription>Failed to fetch task list</AlertDescription>
          </Alert>
        ) : null}

        {tasksLoading ? (
          <StageEmptyState
            icon={Loader2}
            title="Loading task list"
            description="Fetching latest task status from the backend."
            className="[&_svg]:animate-spin"
          />
        ) : filteredTasks.length === 0 ? (
          <StageEmptyState
            icon={ListChecks}
            title="No tasks yet"
            description="Go to Audio, Image, or Video pages to create a task."
          />
        ) : (
          <SoftPanel className="overflow-hidden p-3">
            <div className="workspace-soft-panel overflow-x-auto rounded-[26px] p-2">
              <table className="w-full min-w-[980px] text-sm">
                <thead>
                  <tr className="border-b border-border/60 text-left text-xs uppercase tracking-[0.12em] text-muted-foreground">
                    <th className="px-4 py-3">Task ID</th>
                    <th className="px-4 py-3">Type</th>
                    <th className="px-4 py-3">Status</th>
                    <th className="px-4 py-3">Created</th>
                    <th className="px-4 py-3">Updated</th>
                    <th className="px-4 py-3 text-right">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredTasks.map((task) => (
                    <tr key={task.id} className="border-b border-border/45 hover:bg-[var(--surface-1)]/60">
                      <td className="max-w-[240px] truncate px-4 py-4 font-mono text-xs font-medium" title={task.id}>
                        {task.id}
                      </td>
                      <td className="max-w-[180px] truncate px-4 py-4" title={task.task_type}>
                        {task.task_type}
                      </td>
                      <td className="px-4 py-4">{renderStatusPill(task.status)}</td>
                      <td className="px-4 py-4 text-muted-foreground">{formatDateTime(task.created_at)}</td>
                      <td className="px-4 py-4 text-muted-foreground">{formatDateTime(task.updated_at)}</td>
                      <td className="px-4 py-4 text-right">
                        <Dialog>
                          <DialogTrigger asChild>
                            <Button
                              variant="pill"
                              size="sm"
                              onClick={() => {
                                setSelectedTask(null);
                                void fetchTaskDetail(task.id);
                              }}
                            >
                              Details
                            </Button>
                          </DialogTrigger>
                          <DialogContent className="max-w-3xl">
                            <DialogHeader>
                              <DialogTitle>Task Details</DialogTitle>
                              <DialogDescription>Task ID: {selectedTask?.id ?? task.id}</DialogDescription>
                            </DialogHeader>
                            <div className="space-y-4 py-2">
                              {selectedTask ? (
                                <>
                                  <SoftPanel className="space-y-3 rounded-[20px]">
                                    <div className="grid gap-3 md:grid-cols-2">
                                      <div>
                                        <p className="text-xs uppercase tracking-[0.12em] text-muted-foreground">Type</p>
                                        <p className="mt-1 text-sm font-medium">{selectedTask.task_type}</p>
                                      </div>
                                      <div>
                                        <p className="text-xs uppercase tracking-[0.12em] text-muted-foreground">Status</p>
                                        <div className="mt-1">{renderStatusPill(selectedTask.status)}</div>
                                      </div>
                                      <div>
                                        <p className="text-xs uppercase tracking-[0.12em] text-muted-foreground">Created</p>
                                        <p className="mt-1 text-sm font-medium">{new Date(selectedTask.created_at).toLocaleString()}</p>
                                      </div>
                                      <div>
                                        <p className="text-xs uppercase tracking-[0.12em] text-muted-foreground">Updated</p>
                                        <p className="mt-1 text-sm font-medium">{new Date(selectedTask.updated_at).toLocaleString()}</p>
                                      </div>
                                    </div>
                                  </SoftPanel>

                                  {selectedTask.status === 'failed' && selectedTask.error_msg ? (
                                    <Alert variant="destructive">
                                      <AlertTitle>Failure reason</AlertTitle>
                                      <AlertDescription className="whitespace-pre-wrap break-words">
                                        {selectedTask.error_msg}
                                      </AlertDescription>
                                    </Alert>
                                  ) : null}

                                  {selectedTask.status === 'succeeded' && taskResult ? (
                                    <SoftPanel className="space-y-3 rounded-[20px]">
                                      <h4 className="text-sm font-semibold uppercase tracking-[0.1em] text-muted-foreground">
                                        Task Result
                                      </h4>
                                      {taskResult.text ? (
                                        <div className="space-y-3">
                                          <p className="whitespace-pre-wrap text-sm leading-6">{taskResult.text}</p>
                                          <Button
                                            variant="pill"
                                            size="sm"
                                            onClick={() => {
                                              navigator.clipboard.writeText(taskResult.text);
                                              toast.success('Copied to clipboard');
                                            }}
                                          >
                                            Copy result
                                          </Button>
                                        </div>
                                      ) : (
                                        <pre className="overflow-x-auto rounded-xl bg-[var(--surface-1)] p-3 text-xs">
                                          {JSON.stringify(taskResult, null, 2)}
                                        </pre>
                                      )}
                                    </SoftPanel>
                                  ) : null}

                                  <div className="flex flex-wrap gap-2">
                                    {selectedTask.status === 'running' ? (
                                      <Button
                                        variant="destructive"
                                        size="sm"
                                        onClick={() => void cancelTask(selectedTask.id)}
                                        disabled={cancelTaskMutation.isPending}
                                      >
                                        {cancelTaskMutation.isPending ? 'Cancelling...' : 'Cancel task'}
                                      </Button>
                                    ) : null}
                                    {selectedTask.status === 'failed' ||
                                    selectedTask.status === 'cancelled' ||
                                    selectedTask.status === 'succeeded' ? (
                                      <Button
                                        variant="pill"
                                        size="sm"
                                        onClick={() => void restartTask(selectedTask.id)}
                                        disabled={restartTaskMutation.isPending}
                                      >
                                        {restartTaskMutation.isPending ? 'Restarting...' : 'Restart task'}
                                      </Button>
                                    ) : null}
                                  </div>
                                </>
                              ) : (
                                <div className="flex justify-center py-10">
                                  <Spinner className="h-8 w-8" />
                                </div>
                              )}
                            </div>
                          </DialogContent>
                        </Dialog>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </SoftPanel>
        )}
      </div>
    </div>
  );
}

function renderStatusPill(status: string) {
  if (status === 'succeeded') {
    return <StatusPill status="success">Succeeded</StatusPill>;
  }
  if (status === 'running') {
    return <StatusPill status="info">Running</StatusPill>;
  }
  if (status === 'failed') {
    return <StatusPill status="danger">Failed</StatusPill>;
  }
  if (status === 'cancelled') {
    return <StatusPill status="neutral">Cancelled</StatusPill>;
  }
  if (status === 'pending') {
    return <StatusPill status="neutral">Pending</StatusPill>;
  }

  return <StatusPill status="neutral">{status}</StatusPill>;
}

function formatDateTime(value: string) {
  return new Date(value).toLocaleString(undefined, {
    year: '2-digit',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}
