import { useEffect, useMemo, useState } from 'react';
import api from '@/lib/api';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { toast } from 'sonner';
import { Loader2, RefreshCw, Download, Upload, X } from 'lucide-react';

const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

type BusyAction = 'prepare' | 'download' | 'unload' | null;
type StatusFilter = 'all' | 'downloaded' | 'pending' | 'not_downloaded';

type ModelStatus = 'downloaded' | 'pending' | 'not_downloaded';

export default function Hub() {
  const [selectedModelId, setSelectedModelId] = useState('');
  const [selectedBackendId, setSelectedBackendId] = useState('');
  const [numWorkers, setNumWorkers] = useState(1);

  const [searchKeyword, setSearchKeyword] = useState('');
  const [backendFilter, setBackendFilter] = useState('all');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');

  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [busyModelId, setBusyModelId] = useState<string | null>(null);

  const {
    data: catalogModels,
    isLoading: catalogModelsLoading,
    error: catalogModelsError,
    refetch: refetchCatalogModels,
  } = api.useQuery('get', '/v1/models');

  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const loadModelMutation = api.useMutation('post', '/v1/models/load');
  const unloadModelMutation = api.useMutation('post', '/v1/models/unload');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const modelList = catalogModels ?? [];
  const isBusy = busyAction !== null;

  const pendingTaskIdOf = (model: unknown): string | null => {
    const pendingTaskId = (model as { pending_task_id?: string | null }).pending_task_id;
    if (typeof pendingTaskId !== 'string') return null;
    const trimmed = pendingTaskId.trim();
    return trimmed.length > 0 ? trimmed : null;
  };

  const statusOfModel = (model: { local_path?: string | null }): ModelStatus => {
    if (model.local_path) return 'downloaded';
    if (pendingTaskIdOf(model)) return 'pending';
    return 'not_downloaded';
  };

  const selectedModel = useMemo(
    () => modelList.find((model) => model.id === selectedModelId),
    [modelList, selectedModelId]
  );

  const backendOptions = useMemo(() => {
    const unique = new Set<string>();
    for (const model of modelList) {
      for (const backend of model.backend_ids) {
        unique.add(backend);
      }
    }
    return Array.from(unique).sort();
  }, [modelList]);

  useEffect(() => {
    if (modelList.length === 0) {
      setSelectedModelId('');
      return;
    }
    const exists = modelList.some((model) => model.id === selectedModelId);
    if (!selectedModelId || !exists) {
      setSelectedModelId(modelList[0].id);
    }
  }, [modelList, selectedModelId]);

  useEffect(() => {
    if (!selectedModel) {
      setSelectedBackendId('');
      return;
    }
    const compatible = selectedModel.backend_ids.includes(selectedBackendId);
    if (!selectedBackendId || !compatible) {
      setSelectedBackendId(selectedModel.backend_ids[0] ?? '');
    }
  }, [selectedModel, selectedBackendId]);

  const filteredModels = useMemo(() => {
    const keyword = searchKeyword.trim().toLowerCase();
    return modelList.filter((model) => {
      if (backendFilter !== 'all' && !model.backend_ids.includes(backendFilter)) {
        return false;
      }

      const status = statusOfModel(model);
      if (statusFilter !== 'all' && status !== statusFilter) {
        return false;
      }

      if (!keyword) {
        return true;
      }

      const haystack = [
        model.id,
        model.display_name,
        model.repo_id,
        model.filename,
        model.local_path ?? '',
      ]
        .join(' ')
        .toLowerCase();

      return haystack.includes(keyword);
    });
  }, [backendFilter, modelList, searchKeyword, statusFilter]);

  const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

  const extractTaskId = (payload: unknown): string | null => {
    if (typeof payload !== 'object' || payload === null) return null;
    const taskId = (payload as { task_id?: unknown }).task_id;
    if (typeof taskId !== 'string') return null;
    const trimmed = taskId.trim();
    return trimmed.length > 0 ? trimmed : null;
  };

  const waitForTaskToFinish = async (taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;
    while (Date.now() < deadline) {
      const task = await getTaskMutation.mutateAsync({
        params: {
          path: { id: taskId },
        },
      });

      if (task.status === 'succeeded') {
        return;
      }

      if (task.status === 'failed' || task.status === 'cancelled' || task.status === 'interrupted') {
        throw new Error(task.error_msg ?? `Task ${taskId} ended with status: ${task.status}`);
      }

      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS);
    }

    throw new Error('Model download timed out');
  };

  const refreshCatalogAndFindModel = async (modelId: string) => {
    const refreshed = await refetchCatalogModels();
    const models = refreshed.data ?? [];
    return models.find((model) => model.id === modelId);
  };

  const ensureDownloadedModelPath = async (modelId: string, backendId: string): Promise<string> => {
    let model = modelList.find((item) => item.id === modelId);
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId);
    }
    if (!model) {
      throw new Error('Selected model does not exist in catalog');
    }

    if (model.local_path) {
      return model.local_path;
    }

    let taskId = pendingTaskIdOf(model);
    if (!taskId) {
      const downloadResponse = await downloadModelMutation.mutateAsync({
        body: {
          backend_id: backendId,
          model_id: modelId,
        },
      });
      taskId = extractTaskId(downloadResponse);
    }

    if (!taskId) {
      throw new Error('Failed to start model download task');
    }

    await waitForTaskToFinish(taskId);

    const refreshedModel = await refreshCatalogAndFindModel(modelId);
    if (!refreshedModel?.local_path) {
      throw new Error('Model download completed, but local_path is empty');
    }
    return refreshedModel.local_path;
  };

  const chooseBackendForModel = (model: { backend_ids: string[] }): string => {
    if (selectedBackendId && model.backend_ids.includes(selectedBackendId)) {
      return selectedBackendId;
    }
    return model.backend_ids[0] ?? '';
  };

  const runPrepareAndLoad = async (modelId: string, backendId: string) => {
    const model = modelList.find((item) => item.id === modelId);
    if (!model) {
      toast.error('Model no longer exists');
      return;
    }
    if (!backendId) {
      toast.error('No available backend for this model');
      return;
    }
    if (!model.backend_ids.includes(backendId)) {
      toast.error('Selected backend is not supported by this model');
      return;
    }
    if (!Number.isFinite(numWorkers) || numWorkers < 1) {
      toast.error('Number of workers must be at least 1');
      return;
    }

    setBusyAction('prepare');
    setBusyModelId(modelId);
    try {
      const wasDownloaded = Boolean(model.local_path);
      const modelPath = await ensureDownloadedModelPath(modelId, backendId);
      if (!wasDownloaded) {
        toast.success(`Downloaded ${model.display_name}`);
      }

      await loadModelMutation.mutateAsync({
        body: {
          backend_id: backendId,
          model_path: modelPath,
          num_workers: numWorkers,
        },
      });

      toast.success(`${model.display_name} is ready on ${backendId}`);
      await refetchCatalogModels();
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      toast.error(`Failed to prepare model: ${message}`);
    } finally {
      setBusyAction(null);
      setBusyModelId(null);
    }
  };

  const runDownloadOnly = async (modelId: string, backendId: string) => {
    const model = modelList.find((item) => item.id === modelId);
    if (!model) {
      toast.error('Model no longer exists');
      return;
    }
    if (!backendId) {
      toast.error('No available backend for this model');
      return;
    }
    if (!model.backend_ids.includes(backendId)) {
      toast.error('Selected backend is not supported by this model');
      return;
    }

    setBusyAction('download');
    setBusyModelId(modelId);
    try {
      if (model.local_path) {
        toast.success(`${model.display_name} is already downloaded`);
        return;
      }

      await ensureDownloadedModelPath(modelId, backendId);
      toast.success(`Downloaded ${model.display_name}`);
      await refetchCatalogModels();
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      toast.error(`Failed to download model: ${message}`);
    } finally {
      setBusyAction(null);
      setBusyModelId(null);
    }
  };

  const handleTopPrepare = async () => {
    if (!selectedModel) {
      toast.error('Please select a model');
      return;
    }
    if (!selectedBackendId) {
      toast.error('Please select a backend');
      return;
    }
    await runPrepareAndLoad(selectedModel.id, selectedBackendId);
  };

  const handleTopDownload = async () => {
    if (!selectedModel) {
      toast.error('Please select a model');
      return;
    }
    if (!selectedBackendId) {
      toast.error('Please select a backend');
      return;
    }
    await runDownloadOnly(selectedModel.id, selectedBackendId);
  };

  const handleRowPrepare = async (modelId: string) => {
    const model = modelList.find((item) => item.id === modelId);
    if (!model) return;

    const backendId = chooseBackendForModel(model);
    setSelectedModelId(modelId);
    setSelectedBackendId(backendId);
    await runPrepareAndLoad(modelId, backendId);
  };

  const handleRowDownload = async (modelId: string) => {
    const model = modelList.find((item) => item.id === modelId);
    if (!model) return;

    const backendId = chooseBackendForModel(model);
    setSelectedModelId(modelId);
    setSelectedBackendId(backendId);
    await runDownloadOnly(modelId, backendId);
  };

  const handleUnloadBackend = async () => {
    if (!selectedBackendId) {
      toast.error('Please select a backend first');
      return;
    }

    setBusyAction('unload');
    setBusyModelId(null);
    try {
      await unloadModelMutation.mutateAsync({
        body: {
          backend_id: selectedBackendId,
          model_path: '',
        },
      });
      toast.success(`Unloaded backend ${selectedBackendId}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      toast.error(`Failed to unload backend: ${message}`);
    } finally {
      setBusyAction(null);
      setBusyModelId(null);
    }
  };

  const statusBadge = (status: ModelStatus) => {
    if (status === 'downloaded') return <Badge variant="default">Downloaded</Badge>;
    if (status === 'pending') return <Badge variant="secondary">Downloading</Badge>;
    return <Badge variant="outline">Not downloaded</Badge>;
  };

  return (
    <div className="container mx-auto max-w-6xl space-y-6 px-4 py-8">
      <Card>
        <CardHeader>
          <CardTitle>One-Click Activation</CardTitle>
          <CardDescription>
            Pick a model and backend, then click Prepare & Load. Missing files will be downloaded automatically.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <p className="text-sm font-medium">Model</p>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-xs"
                  onClick={() => refetchCatalogModels()}
                  disabled={catalogModelsLoading || isBusy}
                >
                  {catalogModelsLoading ? (
                    <>
                      <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                      Refreshing
                    </>
                  ) : (
                    <>
                      <RefreshCw className="mr-1 h-3 w-3" />
                      Refresh
                    </>
                  )}
                </Button>
              </div>
              <Select
                value={selectedModelId}
                onValueChange={setSelectedModelId}
                disabled={catalogModelsLoading || isBusy || modelList.length === 0}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Select model" />
                </SelectTrigger>
                <SelectContent>
                  {modelList.length === 0 ? (
                    <div className="px-2 py-1.5 text-sm text-muted-foreground">No models in catalog</div>
                  ) : (
                    modelList.map((model) => (
                      <SelectItem key={model.id} value={model.id}>
                        {model.display_name}
                      </SelectItem>
                    ))
                  )}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <p className="text-sm font-medium">Backend</p>
              <Select
                value={selectedBackendId}
                onValueChange={setSelectedBackendId}
                disabled={isBusy || !selectedModel}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Select backend" />
                </SelectTrigger>
                <SelectContent>
                  {selectedModel ? (
                    selectedModel.backend_ids.map((backendId) => (
                      <SelectItem key={backendId} value={backendId}>
                        {backendId}
                      </SelectItem>
                    ))
                  ) : (
                    <div className="px-2 py-1.5 text-sm text-muted-foreground">Select model first</div>
                  )}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <p className="text-sm font-medium">Workers</p>
              <Input
                type="number"
                min="1"
                value={numWorkers}
                onChange={(event) => {
                  const parsed = Number(event.target.value);
                  if (!Number.isFinite(parsed) || parsed < 1) {
                    setNumWorkers(1);
                    return;
                  }
                  setNumWorkers(Math.floor(parsed));
                }}
                disabled={isBusy}
              />
            </div>
          </div>

          <div className="flex flex-wrap gap-3">
            <Button type="button" onClick={() => void handleTopPrepare()} disabled={isBusy || !selectedModel || !selectedBackendId}>
              {busyAction === 'prepare' ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Preparing...
                </>
              ) : (
                <>
                  <Upload className="mr-2 h-4 w-4" />
                  Prepare & Load
                </>
              )}
            </Button>

            <Button type="button" variant="outline" onClick={() => void handleTopDownload()} disabled={isBusy || !selectedModel || !selectedBackendId}>
              {busyAction === 'download' ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Downloading...
                </>
              ) : (
                <>
                  <Download className="mr-2 h-4 w-4" />
                  Download Only
                </>
              )}
            </Button>

            <Button type="button" variant="destructive" onClick={() => void handleUnloadBackend()} disabled={isBusy || !selectedBackendId}>
              {busyAction === 'unload' ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Unloading...
                </>
              ) : (
                <>
                  <X className="mr-2 h-4 w-4" />
                  Unload Backend
                </>
              )}
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Model Table</CardTitle>
          <CardDescription>
            Use filters to find models quickly. Row actions are integrated with the same one-click workflow.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            <Input
              value={searchKeyword}
              onChange={(event) => setSearchKeyword(event.target.value)}
              placeholder="Search model / repo / file"
              disabled={isBusy}
            />

            <Select value={backendFilter} onValueChange={setBackendFilter} disabled={isBusy}>
              <SelectTrigger>
                <SelectValue placeholder="Filter by backend" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All backends</SelectItem>
                {backendOptions.map((backendId) => (
                  <SelectItem key={backendId} value={backendId}>
                    {backendId}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <Select value={statusFilter} onValueChange={(value) => setStatusFilter(value as StatusFilter)} disabled={isBusy}>
              <SelectTrigger>
                <SelectValue placeholder="Filter by status" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All status</SelectItem>
                <SelectItem value="downloaded">Downloaded</SelectItem>
                <SelectItem value="pending">Downloading</SelectItem>
                <SelectItem value="not_downloaded">Not downloaded</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[220px]">Model</TableHead>
                  <TableHead className="w-[280px]">Repo / File</TableHead>
                  <TableHead className="w-[220px]">Backends</TableHead>
                  <TableHead className="w-[140px]">Status</TableHead>
                  <TableHead>Local Path</TableHead>
                  <TableHead className="w-[220px] text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {catalogModelsError ? (
                  <TableRow>
                    <TableCell colSpan={6} className="text-center text-destructive">
                      Failed to load model catalog
                    </TableCell>
                  </TableRow>
                ) : catalogModelsLoading ? (
                  <TableRow>
                    <TableCell colSpan={6} className="text-center text-muted-foreground">
                      Loading models...
                    </TableCell>
                  </TableRow>
                ) : filteredModels.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={6} className="text-center text-muted-foreground">
                      No models matched the filters
                    </TableCell>
                  </TableRow>
                ) : (
                  filteredModels.map((model) => {
                    const isSelected = selectedModelId === model.id;
                    const modelStatus = statusOfModel(model);
                    const rowBackend = chooseBackendForModel(model);
                    const rowBusy = busyModelId === model.id;

                    return (
                      <TableRow
                        key={model.id}
                        className={isSelected ? 'bg-muted/40' : undefined}
                        onClick={() => {
                          setSelectedModelId(model.id);
                          setSelectedBackendId(rowBackend);
                        }}
                      >
                        <TableCell>
                          <div className="space-y-1">
                            <p className="font-medium">{model.display_name}</p>
                            <p className="truncate font-mono text-xs text-muted-foreground">{model.id}</p>
                          </div>
                        </TableCell>

                        <TableCell>
                          <div className="space-y-1 text-xs text-muted-foreground">
                            <p className="truncate">{model.repo_id}</p>
                            <p className="truncate">{model.filename}</p>
                          </div>
                        </TableCell>

                        <TableCell>
                          <div className="flex flex-wrap gap-1">
                            {model.backend_ids.map((backendId) => (
                              <Badge key={backendId} variant={backendId === rowBackend ? 'default' : 'outline'}>
                                {backendId}
                              </Badge>
                            ))}
                          </div>
                        </TableCell>

                        <TableCell>{statusBadge(modelStatus)}</TableCell>

                        <TableCell className="max-w-[320px] truncate text-xs text-muted-foreground">
                          {model.local_path ?? '-'}
                        </TableCell>

                        <TableCell>
                          <div className="flex justify-end gap-2">
                            <Button
                              type="button"
                              size="sm"
                              onClick={(event) => {
                                event.stopPropagation();
                                void handleRowPrepare(model.id);
                              }}
                              disabled={isBusy}
                            >
                              {rowBusy && busyAction === 'prepare' ? (
                                <>
                                  <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                                  Using...
                                </>
                              ) : (
                                'Use'
                              )}
                            </Button>

                            <Button
                              type="button"
                              size="sm"
                              variant="outline"
                              onClick={(event) => {
                                event.stopPropagation();
                                void handleRowDownload(model.id);
                              }}
                              disabled={isBusy}
                            >
                              {rowBusy && busyAction === 'download' ? (
                                <>
                                  <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                                  Downloading...
                                </>
                              ) : (
                                'Download'
                              )}
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    );
                  })
                )}
              </TableBody>
            </Table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
