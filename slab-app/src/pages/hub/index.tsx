import { useEffect, useMemo, useState } from 'react';
import api, { getErrorMessage } from '@/lib/api';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import { Label } from '@/components/ui/label';
import { Checkbox } from '@/components/ui/checkbox';
import { toast } from 'sonner';
import { Loader2, Pencil, Plus, Trash2 } from 'lucide-react';

const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

type BusyAction = 'download' | null;
type StatusFilter = 'all' | 'downloaded' | 'pending' | 'not_downloaded';

type ModelStatus = 'downloaded' | 'pending' | 'not_downloaded';

type ModelDraft = {
  display_name: string;
  repo_id: string;
  filename: string;
  backend_ids: string[];
};

const EMPTY_MODEL_DRAFT: ModelDraft = {
  display_name: '',
  repo_id: '',
  filename: '',
  backend_ids: [],
};

export default function Hub() {
  const [selectedModelId, setSelectedModelId] = useState('');
  const [selectedBackendId, setSelectedBackendId] = useState('');

  const [searchKeyword, setSearchKeyword] = useState('');
  const [backendFilter, setBackendFilter] = useState('all');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');

  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [busyModelId, setBusyModelId] = useState<string | null>(null);
  const [isModelDialogOpen, setIsModelDialogOpen] = useState(false);
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [modelDraft, setModelDraft] = useState<ModelDraft>(EMPTY_MODEL_DRAFT);
  const [deletingModelId, setDeletingModelId] = useState<string | null>(null);

  const {
    data: catalogModels,
    isLoading: catalogModelsLoading,
    error: catalogModelsError,
    refetch: refetchCatalogModels,
  } = api.useQuery('get', '/v1/models');

  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const createModelMutation = api.useMutation('post', '/v1/models');
  const updateModelMutation = api.useMutation('put', '/v1/models/{id}');
  const deleteModelMutation = api.useMutation('delete', '/v1/models/{id}');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const modelList = catalogModels ?? [];
  const isBusy = busyAction !== null;
  const isSavingModel = createModelMutation.isPending || updateModelMutation.isPending;
  const isCatalogMutating = isSavingModel || deletingModelId !== null;

  const pendingTaskIdOf = (model: unknown): string | null => {
    if (typeof model !== 'object' || model === null) return null;
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

  const availableBackendIds = useMemo(() => {
    const defaults = ['ggml.llama', 'ggml.whisper', 'ggml.diffusion'];
    const extras = backendOptions.filter((backendId) => !defaults.includes(backendId));
    return [...defaults, ...extras];
  }, [backendOptions]);

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

  const resetModelDialog = () => {
    setIsModelDialogOpen(false);
    setEditingModelId(null);
    setModelDraft(EMPTY_MODEL_DRAFT);
  };

  const openCreateModelDialog = () => {
    setEditingModelId(null);
    setModelDraft({
      ...EMPTY_MODEL_DRAFT,
      backend_ids: availableBackendIds.length > 0 ? [availableBackendIds[0]] : [],
    });
    setIsModelDialogOpen(true);
  };

  const openEditModelDialog = (model: {
    id: string;
    display_name: string;
    repo_id: string;
    filename: string;
    backend_ids: string[];
  }) => {
    setEditingModelId(model.id);
    setModelDraft({
      display_name: model.display_name,
      repo_id: model.repo_id,
      filename: model.filename,
      backend_ids: model.backend_ids,
    });
    setIsModelDialogOpen(true);
  };

  const setModelField = (field: keyof ModelDraft, value: string) => {
    setModelDraft((prev) => ({
      ...prev,
      [field]: value,
    }));
  };

  const toggleBackendId = (backendId: string, checked: boolean) => {
    setModelDraft((prev) => {
      if (checked) {
        if (prev.backend_ids.includes(backendId)) return prev;
        return { ...prev, backend_ids: [...prev.backend_ids, backendId] };
      }
      return {
        ...prev,
        backend_ids: prev.backend_ids.filter((id) => id !== backendId),
      };
    });
  };

  const saveModel = async () => {
    const display_name = modelDraft.display_name.trim();
    const repo_id = modelDraft.repo_id.trim();
    const filename = modelDraft.filename.trim();
    const backend_ids = modelDraft.backend_ids;

    if (!display_name || !repo_id || !filename) {
      toast.error('Display name, repository ID, and filename are required.');
      return;
    }
    if (backend_ids.length === 0) {
      toast.error('Select at least one backend.');
      return;
    }

    try {
      if (editingModelId) {
        await updateModelMutation.mutateAsync({
          params: {
            path: { id: editingModelId },
          },
          body: {
            display_name,
            repo_id,
            filename,
            backend_ids,
          },
        });
        toast.success('Model updated');
      } else {
        await createModelMutation.mutateAsync({
          body: {
            display_name,
            repo_id,
            filename,
            backend_ids,
          },
        });
        toast.success('Model added to catalog');
      }
      await refetchCatalogModels();
      resetModelDialog();
    } catch (error) {
      toast.error(getErrorMessage(error));
    }
  };

  const deleteModel = async (id: string) => {
    setDeletingModelId(id);
    try {
      await deleteModelMutation.mutateAsync({
        params: {
          path: { id },
        },
      });
      toast.success('Model removed from catalog');
      await refetchCatalogModels();
      if (editingModelId === id) {
        resetModelDialog();
      }
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setDeletingModelId(null);
    }
  };

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

  const handleRowDownload = async (modelId: string) => {
    const model = modelList.find((item) => item.id === modelId);
    if (!model) return;

    const backendId = chooseBackendForModel(model);
    setSelectedModelId(modelId);
    setSelectedBackendId(backendId);
    await runDownloadOnly(modelId, backendId);
  };

  const statusBadge = (status: ModelStatus) => {
    if (status === 'downloaded') return <Badge variant="default">Downloaded</Badge>;
    if (status === 'pending') return <Badge variant="secondary">Downloading</Badge>;
    return <Badge variant="outline">Not downloaded</Badge>;
  };

  return (
    <div className="container mx-auto max-w-6xl space-y-6 px-4 py-8">
      <Card>
        <CardHeader className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <CardTitle>Model Table</CardTitle>
            <CardDescription>
              Manage model catalog entries and downloads in one place.
            </CardDescription>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={() => refetchCatalogModels()} disabled={catalogModelsLoading || isBusy || isCatalogMutating}>
              {catalogModelsLoading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Refresh
            </Button>
            <Button onClick={openCreateModelDialog} disabled={isBusy || isCatalogMutating}>
              <Plus className="mr-2 h-4 w-4" />
              Add Model
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            <Input
              value={searchKeyword}
              onChange={(event) => setSearchKeyword(event.target.value)}
              placeholder="Search model / repo / file"
              disabled={isBusy || isCatalogMutating}
            />

            <Select value={backendFilter} onValueChange={setBackendFilter} disabled={isBusy || isCatalogMutating}>
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

            <Select value={statusFilter} onValueChange={(value) => setStatusFilter(value as StatusFilter)} disabled={isBusy || isCatalogMutating}>
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
                  <TableHead className="sticky right-0 z-20 w-[360px] border-l bg-background text-right">Actions</TableHead>
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
                    <TableCell colSpan={6} className="py-8 text-center">
                      {modelList.length === 0 ? (
                        <div className="space-y-2">
                          <p className="font-medium">No model entries yet</p>
                          <p className="text-sm text-muted-foreground">
                            Add your first model to make it available for downloads and workflows.
                          </p>
                          <Button onClick={openCreateModelDialog} size="sm" disabled={isBusy || isCatalogMutating}>
                            <Plus className="mr-2 h-4 w-4" />
                            Add First Model
                          </Button>
                        </div>
                      ) : (
                        <p className="text-muted-foreground">No models matched the filters</p>
                      )}
                    </TableCell>
                  </TableRow>
                ) : (
                  filteredModels.map((model) => {
                    const isSelected = selectedModelId === model.id;
                    const modelStatus = statusOfModel(model);
                    const isDownloaded = modelStatus === 'downloaded';
                    const isPending = modelStatus === 'pending';
                    const rowBackend = chooseBackendForModel(model);
                    const rowBusy = busyModelId === model.id;

                    return (
                      <TableRow
                        key={model.id}
                        className={isSelected ? 'group bg-muted/40' : 'group'}
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

                        <TableCell className={`sticky right-0 z-10 border-l group-hover:bg-muted/50 ${isSelected ? 'bg-muted/40' : 'bg-background'}`}>
                          <div className="flex items-center justify-end gap-2">
                            <Button
                              type="button"
                              size="sm"
                              variant="outline"
                              onClick={(event) => {
                                event.stopPropagation();
                                void handleRowDownload(model.id);
                              }}
                              disabled={isBusy || isCatalogMutating || deletingModelId === model.id || isDownloaded || isPending}
                            >
                              {rowBusy && busyAction === 'download' ? (
                                <>
                                  <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                                  Downloading...
                                </>
                              ) : isPending ? (
                                'Downloading...'
                              ) : isDownloaded ? (
                                'Downloaded'
                              ) : (
                                'Download'
                              )}
                            </Button>

                            <Button
                              type="button"
                              size="sm"
                              variant="outline"
                              onClick={(event) => {
                                event.stopPropagation();
                                openEditModelDialog(model);
                              }}
                              disabled={isBusy || isCatalogMutating}
                            >
                              <Pencil className="mr-1 h-3.5 w-3.5" />
                              Edit
                            </Button>

                            <AlertDialog>
                              <AlertDialogTrigger asChild>
                                <Button
                                  type="button"
                                  size="sm"
                                  variant="destructive"
                                  disabled={deletingModelId === model.id || isBusy || isCatalogMutating}
                                  onClick={(event) => event.stopPropagation()}
                                >
                                  {deletingModelId === model.id ? (
                                    <Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" />
                                  ) : (
                                    <Trash2 className="mr-1 h-3.5 w-3.5" />
                                  )}
                                  Delete
                                </Button>
                              </AlertDialogTrigger>
                              <AlertDialogContent size="sm">
                                <AlertDialogHeader>
                                  <AlertDialogTitle>Delete model entry?</AlertDialogTitle>
                                  <AlertDialogDescription>
                                    This will remove <strong>{model.display_name}</strong> from the model catalog.
                                  </AlertDialogDescription>
                                </AlertDialogHeader>
                                <AlertDialogFooter>
                                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                                  <AlertDialogAction
                                    variant="destructive"
                                    onClick={() => void deleteModel(model.id)}
                                  >
                                    Delete
                                  </AlertDialogAction>
                                </AlertDialogFooter>
                              </AlertDialogContent>
                            </AlertDialog>
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

      <Dialog
        open={isModelDialogOpen}
        onOpenChange={(open) => {
          if (!open) {
            resetModelDialog();
          } else {
            setIsModelDialogOpen(true);
          }
        }}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>{editingModelId ? 'Edit Model' : 'Add Model'}</DialogTitle>
            <DialogDescription>
              {editingModelId
                ? 'Update this model catalog entry.'
                : 'Create a model catalog entry for download and workflow selection.'}
            </DialogDescription>
          </DialogHeader>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              void saveModel();
            }}
            className="space-y-4"
          >
            <div className="grid gap-2">
              <Label>Display Name</Label>
              <Input
                value={modelDraft.display_name}
                onChange={(event) => setModelField('display_name', event.target.value)}
                placeholder="Qwen2.5 0.5B Instruct (Q4_K_M)"
              />
            </div>

            <div className="grid gap-2">
              <Label>Repository ID</Label>
              <Input
                value={modelDraft.repo_id}
                onChange={(event) => setModelField('repo_id', event.target.value)}
                placeholder="bartowski/Qwen2.5-0.5B-Instruct-GGUF"
              />
            </div>

            <div className="grid gap-2">
              <Label>Filename</Label>
              <Input
                value={modelDraft.filename}
                onChange={(event) => setModelField('filename', event.target.value)}
                placeholder="Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"
              />
            </div>

            <div className="grid gap-2">
              <Label>Compatible Backends</Label>
              <div className="grid grid-cols-1 gap-2 rounded-md border p-3 sm:grid-cols-2">
                {availableBackendIds.map((backendId) => (
                  <label key={backendId} className="flex items-center gap-2 text-sm">
                    <Checkbox
                      checked={modelDraft.backend_ids.includes(backendId)}
                      onCheckedChange={(checked) => toggleBackendId(backendId, Boolean(checked))}
                    />
                    <span>{backendId}</span>
                  </label>
                ))}
              </div>
            </div>

            <DialogFooter>
              <Button type="button" variant="outline" onClick={resetModelDialog}>
                Cancel
              </Button>
              <Button type="submit" disabled={isSavingModel}>
                {isSavingModel && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                {editingModelId ? 'Save Changes' : 'Add Model'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
