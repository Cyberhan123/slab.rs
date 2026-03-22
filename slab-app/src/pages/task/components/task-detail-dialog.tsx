import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { Spinner } from '@/components/ui/spinner';
import { SoftPanel } from '@/components/ui/workspace';
import { toast } from 'sonner';
import type { Task, TaskResult } from '../const';
import { renderStatusPill } from './task-status-pill';

type TaskDetailDialogProps = {
  task: Task;
  selectedTask: Task | null;
  taskResult: TaskResult | null;
  cancelTaskMutation: { isPending: boolean };
  restartTaskMutation: { isPending: boolean };
  onOpen: (id: string) => void;
  onCancel: (id: string) => void;
  onRestart: (id: string) => void;
};

export function TaskDetailDialog({
  task,
  selectedTask,
  taskResult,
  cancelTaskMutation,
  restartTaskMutation,
  onOpen,
  onCancel,
  onRestart,
}: TaskDetailDialogProps) {
  return (
    <Dialog>
      <DialogTrigger asChild>
        <Button
          variant="quiet"
          size="sm"
          className="h-auto rounded-xl px-2 py-1 text-sm font-semibold text-[var(--brand-teal)] hover:bg-[var(--brand-teal)]/6 hover:text-[var(--brand-teal)]"
          onClick={() => {
            onOpen(task.id);
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
                    onClick={() => void onCancel(selectedTask.id)}
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
                    onClick={() => void onRestart(selectedTask.id)}
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
  );
}
