import { useState, useEffect } from 'react';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Spinner } from '@/components/ui/spinner';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { toast } from 'sonner';
import api from '@/lib/api';

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
  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [taskResult, setTaskResult] = useState<TaskResult | null>(null);
  const [taskType, setTaskType] = useState<string>('all');

  // API queries and mutations
  const { data: tasks, error: tasksError, isLoading: tasksLoading, refetch: refetchTasks } = api.useQuery('get', '/v1/tasks', {
    params: {
      query: taskType !== 'all' ? { type: taskType } : undefined
    }
  });
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');
  const getTaskResultMutation = api.useMutation('get', '/v1/tasks/{id}/result');
  const cancelTaskMutation = api.useMutation('post', '/v1/tasks/{id}/cancel');
  const restartTaskMutation = api.useMutation('post', '/v1/tasks/{id}/restart');

  // Auto-refresh for running tasks in the list
  useEffect(() => {
    const hasRunningTasks = tasks?.some(task => task.status === 'running');
    if (!hasRunningTasks) return;

    const interval = setInterval(() => {
      refetchTasks();
    }, 3000); // Poll every 3 seconds

    return () => clearInterval(interval);
  }, [tasks, refetchTasks]);

  // Auto-refresh for selected running task
  useEffect(() => {
    if (!selectedTask || selectedTask.status !== 'running') return;

    const interval = setInterval(() => {
      fetchTaskDetail(selectedTask.id);
    }, 3000); // Poll every 3 seconds

    return () => clearInterval(interval);
  }, [selectedTask?.status, selectedTask?.id]);

  // Show error toast when tasksError changes

  const fetchTaskDetail = async (id: string) => {
    try {
      const data = await getTaskMutation.mutateAsync({
        params: {
          path: { id }
        }
      });

      setSelectedTask(data);

      if (data.status === 'succeeded') {
        await fetchTaskResult(id);
      }
    } catch (err) {
      toast.error('Failed to fetch task details');
    }
  };

  const fetchTaskResult = async (id: string) => {
    try {
      const data = await getTaskResultMutation.mutateAsync({
        params: {
          path: { id }
        }
      });

      setTaskResult(data);
    } catch (err) {
      toast.error('Failed to fetch task result: ' + (err instanceof Error ? err.message : 'Unknown error'));
    }
  };

  const cancelTask = async (id: string) => {
    try {
      await cancelTaskMutation.mutateAsync({
        params: {
          path: { id }
        }
      });

      refetchTasks();
      if (selectedTask?.id === id) {
        await fetchTaskDetail(id);
      }
    } catch (err) {
      toast.error('Failed to cancel task: ' + (err instanceof Error ? err.message : 'Unknown error'));
    }
  };

  const restartTask = async (id: string) => {
    try {
      await restartTaskMutation.mutateAsync({
        params: {
          path: { id }
        }
      });

      refetchTasks();
      if (selectedTask?.id === id) {
        await fetchTaskDetail(id);
      }
    } catch (err) {
      toast.error('Failed to restart task: ' + (err instanceof Error ? err.message : 'Unknown error'));
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'pending':
        return <Badge variant="secondary">Pending</Badge>;
      case 'running':
        return <Badge variant="outline">Running</Badge>;
      case 'succeeded':
        return <Badge variant="default">Succeeded</Badge>;
      case 'failed':
        return <Badge variant="destructive">Failed</Badge>;
      case 'cancelled':
        return <Badge variant="outline">Cancelled</Badge>;
      default:
        return <Badge variant="secondary">{status}</Badge>;
    }
  };

  const formatDateTime = (value: string) =>
    new Date(value).toLocaleString(undefined, {
      year: '2-digit',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });

  return (
    <div className="h-full overflow-auto">
      <div className="container mx-auto space-y-8 px-4 py-8">
        <div className="space-y-4 text-center">
          <h1 className="text-center text-3xl font-bold">Task Management</h1>
          <p className="mx-auto max-w-2xl text-muted-foreground">
            View and manage system tasks
          </p>
        </div>

        <Card>
          <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <CardTitle>Task List</CardTitle>
              <CardDescription>Status and details for all system tasks</CardDescription>
            </div>
            <div className="w-full sm:w-56">
              <Select value={taskType} onValueChange={setTaskType}>
                <SelectTrigger>
                  <SelectValue placeholder="Task type" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All types</SelectItem>
                  <SelectItem value="transcription">Audio transcription</SelectItem>
                  <SelectItem value="image_generation">Image generation</SelectItem>
                  <SelectItem value="model_download">Model download</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </CardHeader>
          <CardContent>
            {tasksError && (
              <Alert variant="destructive">
                <AlertTitle>Error</AlertTitle>
                <AlertDescription>Failed to fetch task list</AlertDescription>
              </Alert>
            )}

            {tasksLoading ? (
              <div className="flex justify-center py-8">
                <Spinner className="h-8 w-8" />
              </div>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-[220px]">Task ID</TableHead>
                    <TableHead className="min-w-[140px]">Type</TableHead>
                    <TableHead className="min-w-[120px]">Status</TableHead>
                    <TableHead className="min-w-[140px]">Created At</TableHead>
                    <TableHead className="hidden min-w-[140px] xl:table-cell">Updated At</TableHead>
                    <TableHead className="sticky right-0 z-20 w-[96px] bg-card text-right shadow-[-1px_0_0_hsl(var(--border))]">
                      Actions
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {tasks?.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={6} className="py-8 text-center text-muted-foreground">
                        <div className="flex flex-col items-center space-y-2">
                          <p>No tasks yet</p>
                          <p className="text-sm">Go to the Audio or Image page to create a task</p>
                        </div>
                      </TableCell>
                    </TableRow>
                  ) : (
                    tasks?.map((task) => (
                      <TableRow key={task.id} className="group">
                        <TableCell className="max-w-[220px] truncate font-mono text-xs font-medium" title={task.id}>
                          {task.id}
                        </TableCell>
                        <TableCell className="max-w-[140px] truncate" title={task.task_type}>{task.task_type}</TableCell>
                        <TableCell>{getStatusBadge(task.status)}</TableCell>
                        <TableCell>{formatDateTime(task.created_at)}</TableCell>
                        <TableCell className="hidden xl:table-cell">{formatDateTime(task.updated_at)}</TableCell>
                        <TableCell className="sticky right-0 z-10 bg-card text-right shadow-[-1px_0_0_hsl(var(--border))] group-hover:bg-muted/50">
                          <Dialog>
                            <DialogTrigger asChild>
                              <Button
                                variant="secondary"
                                size="sm"
                                onClick={() => fetchTaskDetail(task.id)}
                              >
                                Details
                              </Button>
                            </DialogTrigger>
                            <DialogContent className="sm:max-w-[600px]">
                              <DialogHeader>
                                <DialogTitle>Task Details</DialogTitle>
                                <DialogDescription>
                                  Task ID: {selectedTask?.id}
                                </DialogDescription>
                              </DialogHeader>
                              <div className="space-y-4 py-4">
                                {selectedTask ? (
                                  <>
                                    <div className="space-y-2">
                                      <h4 className="font-medium">Basic Info</h4>
                                      <div className="grid grid-cols-2 gap-4">
                                        <div>
                                          <p className="text-sm text-muted-foreground">Type</p>
                                          <p>{selectedTask.task_type}</p>
                                        </div>
                                        <div>
                                          <p className="text-sm text-muted-foreground">Status</p>
                                          <p>{getStatusBadge(selectedTask.status)}</p>
                                        </div>
                                        <div>
                                          <p className="text-sm text-muted-foreground">Created At</p>
                                          <p>{new Date(selectedTask.created_at).toLocaleString()}</p>
                                        </div>
                                        <div>
                                          <p className="text-sm text-muted-foreground">Updated At</p>
                                          <p>{new Date(selectedTask.updated_at).toLocaleString()}</p>
                                        </div>
                                      </div>
                                    </div>

                                    {selectedTask.status === 'failed' && selectedTask.error_msg && (
                                      <Alert variant="destructive">
                                        <AlertTitle>Failure reason</AlertTitle>
                                        <AlertDescription className="whitespace-pre-wrap break-words">
                                          {selectedTask.error_msg}
                                        </AlertDescription>
                                      </Alert>
                                    )}

                                    {selectedTask.status === 'succeeded' && taskResult && (
                                      <div className="space-y-2">
                                        <h4 className="font-medium">Task Result</h4>
                                        <div className="rounded-md border bg-muted/50 p-4">
                                          {taskResult.text ? (
                                            <div className="space-y-2">
                                              <p className="whitespace-pre-wrap text-sm">{taskResult.text}</p>
                                              <Button
                                                variant="secondary"
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
                                            <pre className="whitespace-pre-wrap text-sm">
                                              {JSON.stringify(taskResult, null, 2)}
                                            </pre>
                                          )}
                                        </div>
                                      </div>
                                    )}

                                    <div className="flex space-x-2">
                                      {selectedTask.status === 'running' && (
                                        <Button
                                          variant="destructive"
                                          size="sm"
                                          onClick={() => cancelTask(selectedTask.id)}
                                          disabled={cancelTaskMutation.isPending}
                                        >
                                          {cancelTaskMutation.isPending ? 'Cancelling...' : 'Cancel task'}
                                        </Button>
                                      )}
                                      {(selectedTask.status === 'failed' || selectedTask.status === 'cancelled' || selectedTask.status === 'succeeded') && (
                                        <Button
                                          variant="secondary"
                                          size="sm"
                                          onClick={() => restartTask(selectedTask.id)}
                                          disabled={restartTaskMutation.isPending}
                                        >
                                          {restartTaskMutation.isPending ? 'Restarting...' : 'Restart task'}
                                        </Button>
                                      )}
                                    </div>
                                  </>
                                ) : (
                                  <div className="flex justify-center py-8">
                                    <Spinner className="h-8 w-8" />
                                  </div>
                                )}
                              </div>
                            </DialogContent>
                          </Dialog>
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            )}
          </CardContent>
          <CardFooter className="flex justify-end">
            <Button
              onClick={() => {
                refetchTasks();
              }}
              disabled={tasksLoading}
            >
              {tasksLoading ? 'Refreshing...' : 'Refresh list'}
            </Button>
          </CardFooter>
        </Card>
      </div>
    </div>
  );
}
