import {
  ChevronLeft,
  ChevronRight,
  ListChecks,
  Loader2,
  PlayCircle,
  Timer,
} from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@slab/components/table';
import {
  StageEmptyState,
} from '@slab/components/workspace';

import { useTaskList } from './hooks/use-task-list';
import { formatCompactDuration, formatDateTime, formatPercent, formatTaskId, getTaskTypeMeta } from './utils';
import { TaskMetricCard } from './components/task-metric-card';
import { PaginationButton } from './components/pagination-button';
import { TaskDetailDialog } from './components/task-detail-dialog';
import { renderStatusPill } from './components/task-status-pill';

export default function Task() {
  const {
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
  } = useTaskList();

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
                  className="flex-1 rounded-t-[2px] bg-[var(--brand-teal)]/10"
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
            className="border border-[var(--brand-teal)]/8"
          >
            <div className="mt-5">
              <div className="h-1.5 overflow-hidden rounded-full bg-border/50">
                <div
                  className="h-full rounded-full bg-[var(--brand-gold)] shadow-[0_0_12px_color-mix(in_oklab,var(--brand-gold)_30%,transparent)]"
                  style={{ width: `${Math.max(activeShare, activeTaskCount > 0 ? 8 : 0)}%` }}
                />
              </div>
              <div className="mt-3 flex items-center justify-between font-mono text-[10px] text-muted-foreground">
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
                    className="w-full rounded-[2px] bg-foreground/10"
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
          <section className="overflow-hidden rounded-[20px] border border-border/40 bg-[var(--surface-1)] shadow-[0_12px_40px_-12px_color-mix(in_oklab,var(--foreground)_5%,transparent)]">
            <Table className="min-w-[980px]" variant="roomy">
              <TableHeader className="[&_tr]:border-b-0 [&_tr]:bg-[var(--surface-soft)]">
                <TableRow className="hover:bg-[var(--surface-soft)]">
                  <TableHead className="h-[45px] px-6 text-[11px] font-semibold uppercase tracking-[0.1em] text-muted-foreground">
                    Task ID
                  </TableHead>
                  <TableHead className="h-[45px] px-6 text-[11px] font-semibold uppercase tracking-[0.1em] text-muted-foreground">
                    Type
                  </TableHead>
                  <TableHead className="h-[45px] px-6 text-[11px] font-semibold uppercase tracking-[0.1em] text-muted-foreground">
                    Status
                  </TableHead>
                  <TableHead className="h-[45px] px-6 text-[11px] font-semibold uppercase tracking-[0.1em] text-muted-foreground">
                    Created At
                  </TableHead>
                  <TableHead className="h-[45px] px-6 text-right text-[11px] font-semibold uppercase tracking-[0.1em] text-muted-foreground">
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
                      className="border-b border-border/50 hover:bg-[var(--surface-soft)]"
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
                          <span className="text-sm font-semibold text-foreground">
                            {taskMeta.label}
                          </span>
                        </div>
                      </TableCell>
                      <TableCell className="px-6 py-5">
                        {renderStatusPill(task.status)}
                      </TableCell>
                      <TableCell className="px-6 py-5 text-sm text-muted-foreground">
                        {formatDateTime(task.created_at)}
                      </TableCell>
                      <TableCell className="px-6 py-5 text-right">
                        <TaskDetailDialog
                          task={task}
                          selectedTask={selectedTask}
                          taskResult={taskResult}
                          cancelTaskMutation={cancelTaskMutation}
                          restartTaskMutation={restartTaskMutation}
                          onOpen={(id) => {
                            setSelectedTask(null);
                            void fetchTaskDetail(id);
                          }}
                          onCancel={cancelTask}
                          onRestart={restartTask}
                        />
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>

            <div className="flex flex-wrap items-center justify-between gap-4 border-t border-border/10 bg-[var(--surface-soft)] px-6 py-4">
              <p className="text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground">
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
