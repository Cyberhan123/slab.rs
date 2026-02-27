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
      
      // 获取任务结果
      if (data.status === 'completed') {
        await fetchTaskResult(id);
      }
    } catch (err) {
      toast.error('获取任务详情失败');
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
      toast.error('获取任务结果失败: ' + (err instanceof Error ? err.message : '未知错误'));
    }
  };

  const cancelTask = async (id: string) => {
    try {
      await cancelTaskMutation.mutateAsync({
        params: {
          path: { id }
        }
      });

      // 刷新任务列表
      refetchTasks();
      // 刷新当前选中的任务
      if (selectedTask?.id === id) {
        await fetchTaskDetail(id);
      }
    } catch (err) {
      toast.error('取消任务失败: ' + (err instanceof Error ? err.message : '未知错误'));
    }
  };

  const restartTask = async (id: string) => {
    try {
      await restartTaskMutation.mutateAsync({
        params: {
          path: { id }
        }
      });

      // 刷新任务列表
      refetchTasks();
      // 刷新当前选中的任务
      if (selectedTask?.id === id) {
        await fetchTaskDetail(id);
      }
    } catch (err) {
      toast.error('重启任务失败: ' + (err instanceof Error ? err.message : '未知错误'));
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'pending':
        return <Badge variant="secondary">待处理</Badge>;
      case 'running':
        return <Badge variant="outline">运行中</Badge>;
      case 'completed':
        return <Badge variant="default">已完成</Badge>;
      case 'failed':
        return <Badge variant="destructive">失败</Badge>;
      case 'cancelled':
        return <Badge variant="outline">已取消</Badge>;
      default:
        return <Badge variant="secondary">{status}</Badge>;
    }
  };

  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <div className="text-center space-y-4">
        <h1 className="text-3xl font-bold text-center">任务管理</h1>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          查看和管理系统中的任务
        </p>
      </div>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <div>
            <CardTitle>任务列表</CardTitle>
            <CardDescription>所有系统任务的状态和详情</CardDescription>
          </div>
          <div className="w-48">
            <Select value={taskType} onValueChange={setTaskType}>
              <SelectTrigger>
                <SelectValue placeholder="任务类型" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">所有类型</SelectItem>
                <SelectItem value="transcription">音频转录</SelectItem>
                <SelectItem value="image_generation">图像生成</SelectItem>
                <SelectItem value="model_download">模型下载</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardHeader>
        <CardContent>
          {tasksError && (
            <Alert variant="destructive">
              <AlertTitle>错误</AlertTitle>
              <AlertDescription>获取任务列表失败</AlertDescription>
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
                  <TableHead>任务 ID</TableHead>
                  <TableHead>类型</TableHead>
                  <TableHead>状态</TableHead>
                  <TableHead>创建时间</TableHead>
                  <TableHead>更新时间</TableHead>
                  <TableHead>操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {tasks?.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={6} className="text-center text-muted-foreground py-8">
                      <div className="flex flex-col items-center space-y-2">
                        <p>暂无任务</p>
                        <p className="text-sm">前往音频或图像页面创建任务</p>
                      </div>
                    </TableCell>
                  </TableRow>
                ) : (
                  tasks?.map((task) => (
                    <TableRow key={task.id}>
                      <TableCell className="font-medium">{task.id}</TableCell>
                      <TableCell>{task.task_type}</TableCell>
                      <TableCell>{getStatusBadge(task.status)}</TableCell>
                      <TableCell>{new Date(task.created_at).toLocaleString()}</TableCell>
                      <TableCell>{new Date(task.updated_at).toLocaleString()}</TableCell>
                      <TableCell className="space-x-2">
                        <Dialog>
                          <DialogTrigger asChild>
                            <Button 
                              variant="secondary" 
                              size="sm"
                              onClick={() => fetchTaskDetail(task.id)}
                            >
                              详情
                            </Button>
                          </DialogTrigger>
                          <DialogContent className="sm:max-w-[600px]">
                            <DialogHeader>
                              <DialogTitle>任务详情</DialogTitle>
                              <DialogDescription>
                                任务 ID: {selectedTask?.id}
                              </DialogDescription>
                            </DialogHeader>
                            <div className="space-y-4 py-4">
                              {selectedTask ? (
                                <>
                                  <div className="space-y-2">
                                    <h4 className="font-medium">基本信息</h4>
                                    <div className="grid grid-cols-2 gap-4">
                                      <div>
                                        <p className="text-sm text-muted-foreground">类型</p>
                                        <p>{selectedTask.task_type}</p>
                                      </div>
                                      <div>
                                        <p className="text-sm text-muted-foreground">状态</p>
                                        <p>{getStatusBadge(selectedTask.status)}</p>
                                      </div>
                                      <div>
                                        <p className="text-sm text-muted-foreground">创建时间</p>
                                        <p>{new Date(selectedTask.created_at).toLocaleString()}</p>
                                      </div>
                                      <div>
                                        <p className="text-sm text-muted-foreground">更新时间</p>
                                        <p>{new Date(selectedTask.updated_at).toLocaleString()}</p>
                                      </div>
                                    </div>
                                  </div>

                                  {selectedTask.status === 'completed' && taskResult && (
                                    <div className="space-y-2">
                                      <h4 className="font-medium">任务结果</h4>
                                      <div className="p-4 border rounded-md bg-muted/50">
                                        {taskResult.text ? (
                                          <div className="space-y-2">
                                            <p className="whitespace-pre-wrap text-sm">{taskResult.text}</p>
                                            <Button
                                              variant="secondary"
                                              size="sm"
                                              onClick={() => {
                                                navigator.clipboard.writeText(taskResult.text);
                                                toast.success('已复制到剪贴板');
                                              }}
                                            >
                                              复制结果
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
                                        {cancelTaskMutation.isPending ? '取消中...' : '取消任务'}
                                      </Button>
                                    )}
                                    {(selectedTask.status === 'failed' || selectedTask.status === 'cancelled' || selectedTask.status === 'completed') && (
                                      <Button
                                        variant="secondary"
                                        size="sm"
                                        onClick={() => restartTask(selectedTask.id)}
                                        disabled={restartTaskMutation.isPending}
                                      >
                                        {restartTaskMutation.isPending ? '重启中...' : '重启任务'}
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
            {tasksLoading ? '刷新中...' : '刷新列表'}
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}