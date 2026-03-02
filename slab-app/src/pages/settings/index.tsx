import { useMemo, useState } from 'react';
import api, { getErrorMessage } from "@/lib/api";
import type { paths } from "@/lib/api";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui/tabs';
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
import {
  Label,
} from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Checkbox } from '@/components/ui/checkbox';
import { toast } from 'sonner';
import {
  AlertCircle,
  CheckCircle2,
  Loader2,
  Pencil,
  Plus,
  Trash2,
  XCircle,
} from 'lucide-react';

interface BackendListItem {
  model_type: string;
  backend: string;
  status: string;
}

type ModelCatalogItem =
  paths["/admin/models"]["get"]["responses"][200]["content"]["application/json"][number];

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

function formatDate(value?: string | null) {
  if (!value) return 'Never';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return parsed.toLocaleString();
}

export default function Settings() {
  const [selectedConfigKey, setSelectedConfigKey] = useState<string | null>(null);
  const [configName, setConfigName] = useState<string>('');
  const [configValue, setConfigValue] = useState<string>('');
  const [downloadingBackend, setDownloadingBackend] = useState<string | null>(null);

  const [isModelDialogOpen, setIsModelDialogOpen] = useState(false);
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [modelDraft, setModelDraft] = useState<ModelDraft>(EMPTY_MODEL_DRAFT);
  const [deletingModelId, setDeletingModelId] = useState<string | null>(null);

  // API calls using react-query
  const { data: configs, error: configsError, isLoading: configsLoading, refetch: refetchConfigs } = api.useQuery('get', '/admin/config');
  const { data: backends, error: backendsError, isLoading: backendsLoading } = api.useQuery('get', '/admin/backends');
  const { data: models, error: modelsError, isLoading: modelsLoading, refetch: refetchModels } = api.useQuery('get', '/admin/models');

  // Mutations
  const updateConfigMutation = api.useMutation('put', '/admin/config/{key}');
  const getBackendStatusMutation = api.useMutation('get', '/admin/backends/status');
  const createModelMutation = api.useMutation('post', '/admin/models');
  const updateModelMutation = api.useMutation('put', '/admin/models/{id}');
  const deleteModelMutation = api.useMutation('delete', '/admin/models/{id}');

  const backendList =
    typeof backends === 'object' &&
    backends !== null &&
    Array.isArray((backends as { backends?: unknown }).backends)
      ? ((backends as { backends: BackendListItem[] }).backends ?? [])
      : [];

  const availableBackendIds = useMemo(() => {
    const fromApi = backendList.map((backend) => backend.backend).filter((backend) => backend.startsWith('ggml.'));
    const unique = Array.from(new Set(fromApi));
    if (unique.length > 0) return unique;
    return ['ggml.llama', 'ggml.whisper', 'ggml.diffusion'];
  }, [backendList]);

  const modelList: ModelCatalogItem[] = Array.isArray(models) ? models : [];
  const installedModelCount = modelList.filter((model) => Boolean(model.local_path)).length;

  const isSavingModel = createModelMutation.isPending || updateModelMutation.isPending;

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

  const openEditModelDialog = (model: ModelCatalogItem) => {
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
      await refetchModels();
      resetModelDialog();
    } catch (error) {
      toast.error(getErrorMessage(error));
    }
  };

  // Function to update config value
  const updateConfig = async (key: string, value: string, name?: string) => {
    try {
      await updateConfigMutation.mutateAsync({
        params: {
          path: { key },
        },
        body: {
          name,
          value,
        },
      });
      toast.success('Configuration updated successfully');
      refetchConfigs();
    } catch (error) {
      toast.error(getErrorMessage(error));
    }
  };

  // Function to get backend status
  const getBackendStatus = async (backendId: string) => {
    try {
      const status = await getBackendStatusMutation.mutateAsync({
        params: {
          query: { backend_id: backendId },
        },
      });
      toast.success(`Backend Status: ${status?.status}`);
    } catch (error) {
      toast.error(getErrorMessage(error));
    }
  };

  // Function to download backend
  const downloadBackend = async (backendId: string) => {
    setDownloadingBackend(backendId);
    try {
      toast.warning(
        `Backend ${backendId} download requires backend_id and target_dir. Use /admin/backends/download with payload like {"backend_id":"ggml.llama","target_dir":"C:\\\\slab\\\\llama"}.`
      );
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setDownloadingBackend(null);
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
      await refetchModels();
      if (editingModelId === id) {
        resetModelDialog();
      }
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setDeletingModelId(null);
    }
  };

  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <h1 className="text-3xl font-bold">Settings</h1>

      <Tabs defaultValue="models">
        <TabsList className="grid w-full grid-cols-3">
          <TabsTrigger value="models">Models</TabsTrigger>
          <TabsTrigger value="config">Configuration</TabsTrigger>
          <TabsTrigger value="backends">Backends</TabsTrigger>
        </TabsList>

        <TabsContent value="models" className="mt-6">
          <Card>
            <CardHeader className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
              <div>
                <CardTitle>Model Catalog</CardTitle>
                <CardDescription>
                  Add models here to make them available in Hub download/load/switch flows.
                </CardDescription>
              </div>
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  onClick={() => refetchModels()}
                  disabled={modelsLoading}
                >
                  {modelsLoading && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                  Refresh
                </Button>
                <Button onClick={openCreateModelDialog}>
                  <Plus className="h-4 w-4 mr-2" />
                  Add Model
                </Button>
              </div>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
                <div className="rounded-lg border p-4">
                  <p className="text-xs text-muted-foreground">Catalog Models</p>
                  <p className="mt-1 text-2xl font-semibold">{modelList.length}</p>
                </div>
                <div className="rounded-lg border p-4">
                  <p className="text-xs text-muted-foreground">Installed Models</p>
                  <p className="mt-1 text-2xl font-semibold">{installedModelCount}</p>
                </div>
                <div className="rounded-lg border p-4">
                  <p className="text-xs text-muted-foreground">Not Downloaded</p>
                  <p className="mt-1 text-2xl font-semibold">{modelList.length - installedModelCount}</p>
                </div>
              </div>

              {modelsLoading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="h-6 w-6 animate-spin" />
                  <span className="ml-2">Loading model catalog...</span>
                </div>
              ) : modelsError ? (
                <div className="text-red-500">Error loading model catalog</div>
              ) : (
                <Table>
                  <TableCaption>Catalog entries available to the Hub page</TableCaption>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Name</TableHead>
                      <TableHead>Repository</TableHead>
                      <TableHead>Filename</TableHead>
                      <TableHead>Backends</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Last Download</TableHead>
                      <TableHead>Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {modelList.length > 0 ? (
                      modelList.map((model) => (
                        <TableRow key={model.id}>
                          <TableCell className="font-medium">{model.display_name}</TableCell>
                          <TableCell className="max-w-[240px] break-all">{model.repo_id}</TableCell>
                          <TableCell className="max-w-[260px] break-all">{model.filename}</TableCell>
                          <TableCell>
                            <div className="flex flex-wrap gap-1">
                              {model.backend_ids.map((backendId) => (
                                <Badge key={backendId} variant="outline">
                                  {backendId}
                                </Badge>
                              ))}
                            </div>
                          </TableCell>
                          <TableCell>
                            {model.local_path ? (
                              <Badge className="bg-green-100 text-green-800 hover:bg-green-100 border border-green-200">
                                Installed
                              </Badge>
                            ) : (
                              <Badge variant="outline">Not Downloaded</Badge>
                            )}
                          </TableCell>
                          <TableCell>{formatDate(model.last_downloaded_at)}</TableCell>
                          <TableCell>
                            <div className="flex items-center gap-2">
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => openEditModelDialog(model)}
                              >
                                <Pencil className="h-3.5 w-3.5 mr-1" />
                                Edit
                              </Button>
                              <AlertDialog>
                                <AlertDialogTrigger asChild>
                                  <Button
                                    variant="destructive"
                                    size="sm"
                                    disabled={deletingModelId === model.id}
                                  >
                                    {deletingModelId === model.id ? (
                                      <Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />
                                    ) : (
                                      <Trash2 className="h-3.5 w-3.5 mr-1" />
                                    )}
                                    Delete
                                  </Button>
                                </AlertDialogTrigger>
                                <AlertDialogContent size="sm">
                                  <AlertDialogHeader>
                                    <AlertDialogTitle>Delete model entry?</AlertDialogTitle>
                                    <AlertDialogDescription>
                                      This will remove <strong>{model.display_name}</strong> from the catalog list used by Hub.
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
                      ))
                    ) : (
                      <TableRow>
                        <TableCell colSpan={7} className="py-8 text-center">
                          <div className="space-y-2">
                            <p className="font-medium">No model entries yet</p>
                            <p className="text-sm text-muted-foreground">
                              Add your first model to make it selectable in Hub.
                            </p>
                            <Button onClick={openCreateModelDialog} size="sm">
                              <Plus className="h-4 w-4 mr-2" />
                              Add First Model
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    )}
                  </TableBody>
                </Table>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="config" className="mt-6">
          <Card>
            <CardHeader>
              <CardTitle>Configuration Management</CardTitle>
              <CardDescription>View and update configuration settings</CardDescription>
            </CardHeader>
            <CardContent>
              {configsLoading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="h-6 w-6 animate-spin" />
                  <span className="ml-2">Loading configurations...</span>
                </div>
              ) : configsError ? (
                <div className="text-red-500">Error loading configurations</div>
              ) : (
                <Table>
                      <TableCaption>List of configuration entries</TableCaption>
                      <TableHeader>
                        <TableRow>
                          <TableHead>Key</TableHead>
                          <TableHead>Value</TableHead>
                          <TableHead>Name</TableHead>
                          <TableHead>Actions</TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                    {(configs?.length ?? 0) > 0 ? (
                      configs?.map((config) => (
                        <TableRow key={config.key}>
                          <TableCell className="font-medium">{config.key}</TableCell>
                          <TableCell>{config.value}</TableCell>
                          <TableCell>{config.name}</TableCell>
                          <TableCell>
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => {
                                setSelectedConfigKey(config.key);
                                setConfigName(config.name);
                                setConfigValue(config.value);
                              }}
                            >
                              Edit
                            </Button>
                          </TableCell>
                        </TableRow>
                      ))
                    ) : (
                      <TableRow>
                        <TableCell colSpan={4} className="text-center py-4">
                          No configuration entries found
                        </TableCell>
                      </TableRow>
                    )}
                  </TableBody>
                </Table>
              )}

              {selectedConfigKey && (
                <Card className="mt-6">
                  <CardHeader>
                    <CardTitle>Edit Configuration: {selectedConfigKey}</CardTitle>
                  </CardHeader>
                  <CardContent>
                    <form
                      onSubmit={(e) => {
                        e.preventDefault();
                        updateConfig(selectedConfigKey, configValue, configName);
                        setSelectedConfigKey(null);
                      }}
                      className="space-y-4"
                    >
                      <div className="grid gap-2">
                        <Label>Name</Label>
                        <Input
                          value={configName}
                          onChange={(e) => setConfigName(e.target.value)}
                          placeholder="Human-readable configuration name"
                        />
                      </div>
                      <div className="grid gap-2">
                        <Label>Value</Label>
                        <Input
                          value={configValue}
                          onChange={(e) => setConfigValue(e.target.value)}
                          placeholder="Enter new value"
                        />
                      </div>
                      <div className="flex gap-2">
                        <Button type="submit">Save</Button>
                        <Button
                          variant="outline"
                          onClick={() => {
                            setSelectedConfigKey(null);
                            setConfigName('');
                          }}
                        >
                          Cancel
                        </Button>
                      </div>
                    </form>
                  </CardContent>
                </Card>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="backends" className="mt-6">
          <Card>
            <CardHeader>
              <CardTitle>Backend Management</CardTitle>
              <CardDescription>View and manage backend services</CardDescription>
            </CardHeader>
            <CardContent>
              {backendsLoading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="h-6 w-6 animate-spin" />
                  <span className="ml-2">Loading backends...</span>
                </div>
              ) : backendsError ? (
                <div className="text-red-500">Error loading backends</div>
              ) : (
                <Table>
                  <TableCaption>List of registered backends</TableCaption>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Backend</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {backendList.length > 0 ? (
                      backendList.map((backend) => {
                        const isDownloading = downloadingBackend === backend.backend;

                        // Determine status badge styling and icon
                        const getStatusBadge = () => {
                          switch (backend.status) {
                            case 'running':
                            case 'ready':
                              return (
                                <Badge variant="outline" className="bg-green-50 text-green-700 border-green-200">
                                  <CheckCircle2 className="w-3 h-3 mr-1" />
                                  Ready
                                </Badge>
                              );
                            case 'stopped':
                            case 'not_configured':
                              return (
                                <Badge variant="outline" className="bg-gray-50 text-gray-700 border-gray-200">
                                  <XCircle className="w-3 h-3 mr-1" />
                                  Not Configured
                                </Badge>
                              );
                            default:
                              return (
                                <Badge variant="outline" className="bg-yellow-50 text-yellow-700 border-yellow-200">
                                  <AlertCircle className="w-3 h-3 mr-1" />
                                  {backend.status}
                                </Badge>
                              );
                          }
                        };

                        return (
                          <TableRow key={backend.backend}>
                            <TableCell className="font-medium">{backend.backend}</TableCell>
                            <TableCell>
                              {getStatusBadge()}
                            </TableCell>
                            <TableCell className="flex gap-2">
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => getBackendStatus(backend.backend)}
                                disabled={isDownloading}
                              >
                                Check Status
                              </Button>
                              {backend.status !== 'running' && backend.status !== 'ready' && (
                                <Button
                                  variant="default"
                                  size="sm"
                                  onClick={() => downloadBackend(backend.backend)}
                                  disabled={isDownloading}
                                >
                                  {isDownloading ? (
                                    <>
                                      <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                      Downloading...
                                    </>
                                  ) : (
                                    'Download'
                                  )}
                                </Button>
                              )}
                            </TableCell>
                          </TableRow>
                        );
                      })
                    ) : (
                      <TableRow>
                        <TableCell colSpan={3} className="text-center py-4">
                          No backends found
                        </TableCell>
                      </TableRow>
                    )}
                  </TableBody>
                </Table>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

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
                ? 'Update this catalog entry used by Hub.'
                : 'Create a catalog entry so users can download and select it from Hub.'}
            </DialogDescription>
          </DialogHeader>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void saveModel();
            }}
            className="space-y-4"
          >
            <div className="grid gap-2">
              <Label>Display Name</Label>
              <Input
                value={modelDraft.display_name}
                onChange={(e) => setModelField('display_name', e.target.value)}
                placeholder="Qwen2.5 0.5B Instruct (Q4_K_M)"
              />
            </div>

            <div className="grid gap-2">
              <Label>Repository ID</Label>
              <Input
                value={modelDraft.repo_id}
                onChange={(e) => setModelField('repo_id', e.target.value)}
                placeholder="bartowski/Qwen2.5-0.5B-Instruct-GGUF"
              />
            </div>

            <div className="grid gap-2">
              <Label>Filename</Label>
              <Input
                value={modelDraft.filename}
                onChange={(e) => setModelField('filename', e.target.value)}
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
                {isSavingModel && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                {editingModelId ? 'Save Changes' : 'Add Model'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
