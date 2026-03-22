import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { useEffect, useMemo, useState } from 'react';
import {
  ChevronLeft,
  ChevronRight,
  Download,
  Image,
  ListChecks,
  Loader2,
  Mic,
  PlayCircle,
  Timer,
} from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { Spinner } from '@/components/ui/spinner';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import {
  SoftPanel,
  StageEmptyState,
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

const PAGE_SIZE = 4;

export default function Task() {
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
      <div className="mx-auto flex w-full max-w-[1200px] flex-col gap-6 px-1 pb-8 pt-6">
        <section className="grid gap-6 xl:grid-cols-3">
          <TaskMetricCard
            label="Success Rate"
            value={formatPercent(successRate)}
            note={settledTasks.length > 0 ? `${metrics.succeeded} successful` : 'No completed tasks'}
            noteTone="success"
            icon={PlayCircle}
          >
            <div className="mt-4 flex h-12 items-end gap-1">
              {successSparkline.map((barHeight, index) => (
                <div
                  key={`${index}-${barHeight}`}
                  className="flex-1 rounded-t-[2px] bg-[rgba(0,104,95,0.1)]"
                  style={{ height: `${Math.max(barHeight * 48, 10)}px` }}
                />
              ))}
            </div>
          </TaskMetricCard>

          <TaskMetricCard
            label="Active Queue"
            value={formatPercent(activeShare)}
            note={activeTaskCount > 0 ? `${activeTaskCount} active` : 'Idle'}
            noteTone="muted"
            icon={Timer}
            className="border border-[rgba(0,104,95,0.08)]"
          >
            <div className="mt-5">
              <div className="h-1.5 overflow-hidden rounded-full bg-[#dfe4e7]">
                <div
                  className="h-full rounded-full bg-[#855300] shadow-[0_0_12px_rgba(254,166,25,0.3)]"
                  style={{ width: `${Math.max(activeShare, activeTaskCount > 0 ? 8 : 0)}%` }}
                />
              </div>
              <div className="mt-3 flex items-center justify-between font-mono text-[10px] text-[#6d7a77]">
                <span>0%</span>
                <span>{activeTaskCount > 0 ? 'Active' : 'Idle'}</span>
                <span>100%</span>
              </div>
            </div>
          </TaskMetricCard>

          <TaskMetricCard
            label="Avg. Turnaround"
            value={formatCompactDuration(averageTurnaroundMs)}
            note={settledTasks.length > 0 ? `${settledTasks.length} settled` : 'No settled tasks'}
            noteTone={metrics.failed > 0 ? 'danger' : 'muted'}
            icon={ListChecks}
          >
            <div className="mt-4 flex h-12 items-end gap-1">
              {durationSparkline.map((barHeight, index) => (
                <div key={`${index}-${barHeight}`} className="flex h-full flex-1 items-end">
                  <div
                    className="w-full rounded-[2px] bg-[rgba(79,93,114,0.1)]"
                    style={{ height: `${Math.max(barHeight * 48, 9)}px` }}
                  />
                </div>
              ))}
            </div>
          </TaskMetricCard>
        </section>

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
        ) : allTasks.length === 0 ? (
          <StageEmptyState
            icon={ListChecks}
            title="No tasks yet"
            description="Go to Audio, Image, or Video pages to create a task."
          />
        ) : (
          <section className="overflow-hidden rounded-[20px] border border-[color:rgba(188,201,198,0.35)] bg-[var(--surface-1)] shadow-[0_12px_40px_-12px_rgba(25,28,30,0.05)]">
            <Table className="min-w-[980px]" variant="roomy">
              <TableHeader className="[&_tr]:border-b-0 [&_tr]:bg-[#f2f4f6]">
                <TableRow className="hover:bg-[#f2f4f6]">
                  <TableHead className="h-[45px] px-6 text-[11px] font-semibold uppercase tracking-[0.1em] text-[#6d7a77]">
                    Task ID
                  </TableHead>
                  <TableHead className="h-[45px] px-6 text-[11px] font-semibold uppercase tracking-[0.1em] text-[#6d7a77]">
                    Type
                  </TableHead>
                  <TableHead className="h-[45px] px-6 text-[11px] font-semibold uppercase tracking-[0.1em] text-[#6d7a77]">
                    Status
                  </TableHead>
                  <TableHead className="h-[45px] px-6 text-[11px] font-semibold uppercase tracking-[0.1em] text-[#6d7a77]">
                    Created At
                  </TableHead>
                  <TableHead className="h-[45px] px-6 text-right text-[11px] font-semibold uppercase tracking-[0.1em] text-[#6d7a77]">
                    Actions
                  </TableHead>
                </TableRow>
              </TableHeader>

              <TableBody>
                {paginatedTasks.map((task) => {
                  const taskMeta = getTaskTypeMeta(task.task_type);

                  return (
                    <TableRow
                      key={task.id}
                      className="border-b border-[rgba(236,238,240,1)] hover:bg-[#fbfcfc]"
                    >
                      <TableCell
                        className="px-6 py-6 font-mono text-sm font-medium text-[var(--brand-teal)]"
                        title={task.id}
                      >
                        {formatTaskId(task.id)}
                      </TableCell>
                      <TableCell className="px-6 py-5">
                        <div className="flex items-center gap-3">
                          <div className={`flex size-8 items-center justify-center rounded-xl ${taskMeta.iconBg}`}>
                            <taskMeta.icon className={`h-4 w-4 ${taskMeta.iconColor}`} />
                          </div>
                          <span className="text-sm font-semibold text-[#191c1e]">
                            {taskMeta.label}
                          </span>
                        </div>
                      </TableCell>
                      <TableCell className="px-6 py-5">
                        {renderStatusPill(task.status)}
                      </TableCell>
                      <TableCell className="px-6 py-5 text-sm text-[#6d7a77]">
                        {formatDateTime(task.created_at)}
                      </TableCell>
                      <TableCell className="px-6 py-5 text-right">
                        <Dialog>
                          <DialogTrigger asChild>
                            <Button
                              variant="quiet"
                              size="sm"
                              className="h-auto rounded-xl px-2 py-1 text-sm font-semibold text-[var(--brand-teal)] hover:bg-[rgba(0,104,95,0.06)] hover:text-[var(--brand-teal)]"
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
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>

            <div className="flex flex-wrap items-center justify-between gap-4 border-t border-[rgba(188,201,198,0.12)] bg-[#f2f4f6] px-6 py-4">
              <p className="text-[11px] font-medium uppercase tracking-[0.08em] text-[#6d7a77]">
                {paginationLabel}
              </p>

              <div className="flex items-center gap-2">
                <PaginationButton
                  aria-label="Previous page"
                  disabled={currentPage === 1}
                  onClick={() => setPage((value) => Math.max(1, value - 1))}
                >
                  <ChevronLeft className="h-4 w-4" />
                </PaginationButton>
                {Array.from({ length: totalPages }, (_, index) => {
                  const pageNumber = index + 1;

                  return (
                    <PaginationButton
                      key={pageNumber}
                      active={pageNumber === currentPage}
                      onClick={() => setPage(pageNumber)}
                    >
                      {pageNumber}
                    </PaginationButton>
                  );
                })}
                <PaginationButton
                  aria-label="Next page"
                  disabled={currentPage === totalPages}
                  onClick={() => setPage((value) => Math.min(totalPages, value + 1))}
                >
                  <ChevronRight className="h-4 w-4" />
                </PaginationButton>
              </div>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function renderStatusPill(status: string) {
  const tone = getStatusTone(status);

  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-[11px] font-bold uppercase tracking-[0.04em] ${tone.className}`}
    >
      <span className={`size-1.5 rounded-full ${tone.dotClassName}`} />
      {tone.label}
    </span>
  );
}

function formatDateTime(value: string) {
  return new Date(value).toLocaleString('en-US', {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  });
}

function formatPercent(value: number) {
  if (!Number.isFinite(value)) {
    return '0.0%';
  }

  return `${value.toFixed(1)}%`;
}

function formatCompactDuration(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return '<1s';
  }

  const seconds = value / 1000;
  if (seconds < 60) {
    return `${seconds < 10 ? seconds.toFixed(1) : Math.round(seconds)}s`;
  }

  const minutes = seconds / 60;
  if (minutes < 60) {
    return `${minutes < 10 ? minutes.toFixed(1) : Math.round(minutes)}m`;
  }

  const hours = minutes / 60;
  return `${hours < 10 ? hours.toFixed(1) : Math.round(hours)}h`;
}

function formatTaskId(value: string) {
  return `#${value.replace(/-/g, '').slice(0, 8).toUpperCase()}`;
}

function getTaskDurationMs(task: Task) {
  const createdAt = Date.parse(task.created_at);
  const updatedAt = Date.parse(task.updated_at);

  if (Number.isNaN(createdAt) || Number.isNaN(updatedAt)) {
    return 0;
  }

  return Math.max(updatedAt - createdAt, 0);
}

function isSettledStatus(status: string) {
  return ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(status);
}

function getSparklineWeight(status: string) {
  switch (status) {
    case 'succeeded':
      return 0.92;
    case 'running':
      return 0.72;
    case 'pending':
      return 0.58;
    case 'failed':
      return 0.4;
    case 'cancelled':
    case 'interrupted':
      return 0.3;
    default:
      return 0.48;
  }
}

function getStatusTone(status: string) {
  switch (status) {
    case 'succeeded':
      return {
        label: 'Succeeded',
        className: 'bg-[#d1fae5] text-[#047857]',
        dotClassName: 'bg-[#10b981]',
      };
    case 'running':
      return {
        label: 'Running',
        className: 'bg-[#dbeafe] text-[#1d4ed8]',
        dotClassName: 'bg-[#3b82f6]',
      };
    case 'failed':
      return {
        label: 'Failed',
        className: 'bg-[#fee2e2] text-[#b91c1c]',
        dotClassName: 'bg-[#ef4444]',
      };
    case 'pending':
      return {
        label: 'Queued',
        className: 'bg-[#e5e7eb] text-[#4b5563]',
        dotClassName: 'bg-[#6b7280]',
      };
    case 'cancelled':
      return {
        label: 'Cancelled',
        className: 'bg-[#e5e7eb] text-[#4b5563]',
        dotClassName: 'bg-[#6b7280]',
      };
    case 'interrupted':
      return {
        label: 'Interrupted',
        className: 'bg-[#e5e7eb] text-[#4b5563]',
        dotClassName: 'bg-[#6b7280]',
      };
    default:
      return {
        label: status,
        className: 'bg-[#e5e7eb] text-[#4b5563]',
        dotClassName: 'bg-[#6b7280]',
      };
  }
}

function getTaskTypeMeta(taskType: string) {
  const normalized = taskType.toLowerCase();

  if (normalized.includes('whisper') || normalized.includes('transcription')) {
    return {
      label: 'Transcription',
      icon: Mic,
      iconBg: 'bg-[#d5e3fd]',
      iconColor: 'text-[#446287]',
    };
  }

  if (normalized.includes('diffusion') || normalized.includes('image')) {
    return {
      label: 'Image Generation',
      icon: Image,
      iconBg: 'bg-[#ede9fe]',
      iconColor: 'text-[#6d28d9]',
    };
  }

  if (normalized.includes('download')) {
    return {
      label: 'Model Download',
      icon: Download,
      iconBg: 'bg-[#e0e3e5]',
      iconColor: 'text-[#5b6872]',
    };
  }

  return {
    label: taskType
      .replaceAll('.', ' ')
      .replaceAll('_', ' ')
      .replace(/\b\w/g, (character) => character.toUpperCase()),
    icon: ListChecks,
    iconBg: 'bg-[#e0e3e5]',
    iconColor: 'text-[#5b6872]',
  };
}

type TaskMetricCardProps = {
  label: string;
  value: string;
  note: string;
  noteTone: 'success' | 'danger' | 'muted';
  icon: typeof ListChecks;
  className?: string;
  children: ReactNode;
};

function TaskMetricCard({
  label,
  value,
  note,
  noteTone,
  icon: Icon,
  className,
  children,
}: TaskMetricCardProps) {
  const noteClassName =
    noteTone === 'success'
      ? 'text-[#059669]'
      : noteTone === 'danger'
        ? 'text-[#ef4444]'
        : 'text-[#6d7a77]';

  return (
    <article
      className={`rounded-2xl bg-[#f2f4f6] px-6 py-6 shadow-[0_12px_40px_-24px_rgba(25,28,30,0.08)] ${className ?? ''}`}
    >
      <div className="flex items-start justify-between gap-4">
        <p className="text-[12px] font-bold uppercase tracking-[0.14em] text-[#6d7a77]">
          {label}
        </p>
        <Icon className="h-[18px] w-[18px] text-[#5b6872]" />
      </div>
      <div className="mt-5 flex items-end gap-3">
        <p className="text-[30px] font-semibold leading-none tracking-[-0.03em] text-[#191c1e]">
          {value}
        </p>
        <p className={`pb-1 text-[12px] font-semibold ${noteClassName}`}>
          {note}
        </p>
      </div>
      {children}
    </article>
  );
}

type PaginationButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  active?: boolean;
};

function PaginationButton({ active = false, className, ...props }: PaginationButtonProps) {
  return (
    <button
      type="button"
      className={[
        'flex size-8 items-center justify-center rounded-xl text-xs font-bold transition-colors',
        active
          ? 'bg-[var(--brand-teal)] text-white'
          : 'text-[#191c1e] hover:bg-[rgba(0,104,95,0.08)] disabled:text-[#94a3b8] disabled:hover:bg-transparent',
        className,
      ].join(' ')}
      {...props}
    />
  );
}
