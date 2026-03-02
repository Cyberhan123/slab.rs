import { useEffect, useMemo, useState } from 'react';
import api from "@/lib/api";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui/tabs';
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/components/ui/form';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { toast } from 'sonner';
import { Loader2, Download, Upload, RefreshCw, X } from 'lucide-react';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';

// Schemas for form validation
const downloadSchema = z.object({
  backend_id: z.string().min(1, 'Backend ID is required'),
  model_id: z.string().min(1, 'Catalog model is required'),
});

const loadSchema = z.object({
  model_id: z.string().min(1, 'Installed model is required'),
  backend_id: z.string().min(1, 'Backend ID is required'),
  num_workers: z.number().optional(),
});

const switchSchema = z.object({
  model_id: z.string().min(1, 'Installed model is required'),
  backend_id: z.string().min(1, 'Backend ID is required'),
  num_workers: z.number().optional(),
});

type DownloadFormValues = z.infer<typeof downloadSchema>;
type LoadFormValues = z.infer<typeof loadSchema>;
type SwitchFormValues = z.infer<typeof switchSchema>;

export default function Hub() {
  // State for dialogs
  const [showDownloadDialog, setShowDownloadDialog] = useState(false);
  const [showLoadDialog, setShowLoadDialog] = useState(false);
  const [showSwitchDialog, setShowSwitchDialog] = useState(false);

  // API mutations
  const {
    data: catalogModels,
    isLoading: catalogModelsLoading,
    error: catalogModelsError,
    refetch: refetchCatalogModels,
  } = api.useQuery('get', '/v1/models');
  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const loadModelMutation = api.useMutation('post', '/v1/models/load');
  const switchModelMutation = api.useMutation('post', '/v1/models/switch');
  const unloadModelMutation = api.useMutation('post', '/v1/models/unload');

  // Forms
  const downloadForm = useForm<DownloadFormValues>({
    resolver: zodResolver(downloadSchema),
    defaultValues: {
      backend_id: 'ggml.llama',
      model_id: '',
    },
  });
  const selectedDownloadBackend = downloadForm.watch('backend_id');
  const selectedDownloadModelId = downloadForm.watch('model_id');

  const compatibleCatalogModels = useMemo(() => {
    const models = catalogModels ?? [];
    return models.filter((model) => model.backend_ids.includes(selectedDownloadBackend));
  }, [catalogModels, selectedDownloadBackend]);

  const selectedCatalogModel = useMemo(
    () => compatibleCatalogModels.find((model) => model.id === selectedDownloadModelId),
    [compatibleCatalogModels, selectedDownloadModelId]
  );

  useEffect(() => {
    if (!selectedDownloadModelId) return;
    const stillCompatible = compatibleCatalogModels.some((model) => model.id === selectedDownloadModelId);
    if (!stillCompatible) {
      downloadForm.setValue('model_id', '', { shouldValidate: true });
    }
  }, [compatibleCatalogModels, downloadForm, selectedDownloadModelId]);

  const loadForm = useForm<LoadFormValues>({
    resolver: zodResolver(loadSchema),
    defaultValues: {
      model_id: '',
      backend_id: 'ggml.llama',
      num_workers: 1,
    },
  });

  const switchForm = useForm<SwitchFormValues>({
    resolver: zodResolver(switchSchema),
    defaultValues: {
      model_id: '',
      backend_id: 'ggml.llama',
      num_workers: 1,
    },
  });

  const installedCatalogModels = useMemo(
    () => (catalogModels ?? []).filter((model) => Boolean(model.local_path)),
    [catalogModels]
  );

  const selectedLoadModelId = loadForm.watch('model_id');
  const selectedSwitchModelId = switchForm.watch('model_id');

  const selectedLoadModel = useMemo(
    () => installedCatalogModels.find((model) => model.id === selectedLoadModelId),
    [installedCatalogModels, selectedLoadModelId]
  );
  const selectedSwitchModel = useMemo(
    () => installedCatalogModels.find((model) => model.id === selectedSwitchModelId),
    [installedCatalogModels, selectedSwitchModelId]
  );
  const hasInstalledModels = installedCatalogModels.length > 0;

  const openDownloadDialog = () => {
    setShowLoadDialog(false);
    setShowSwitchDialog(false);
    setShowDownloadDialog(true);
  };

  useEffect(() => {
    if (!selectedLoadModelId) return;
    if (!selectedLoadModel) {
      loadForm.setValue('model_id', '', { shouldValidate: true });
      return;
    }
    const currentBackendId = loadForm.getValues('backend_id');
    if (!selectedLoadModel.backend_ids.includes(currentBackendId)) {
      loadForm.setValue('backend_id', selectedLoadModel.backend_ids[0] ?? '', {
        shouldValidate: true,
      });
    }
  }, [selectedLoadModelId, selectedLoadModel, loadForm]);

  useEffect(() => {
    if (!selectedSwitchModelId) return;
    if (!selectedSwitchModel) {
      switchForm.setValue('model_id', '', { shouldValidate: true });
      return;
    }
    const currentBackendId = switchForm.getValues('backend_id');
    if (!selectedSwitchModel.backend_ids.includes(currentBackendId)) {
      switchForm.setValue('backend_id', selectedSwitchModel.backend_ids[0] ?? '', {
        shouldValidate: true,
      });
    }
  }, [selectedSwitchModelId, selectedSwitchModel, switchForm]);

  // Handlers
  const handleDownloadModel = async (values: DownloadFormValues) => {
    try {
      await downloadModelMutation.mutateAsync({
        body: values,
      });
      toast.success('Model download initiated');
      setShowDownloadDialog(false);
      downloadForm.reset();
      await refetchCatalogModels();
    } catch (error) {
      toast.error('Failed to download model');
    }
  };

  const handleLoadModel = async (values: LoadFormValues) => {
    const selectedModel = installedCatalogModels.find((model) => model.id === values.model_id);
    const modelPath = selectedModel?.local_path;
    if (!selectedModel || !modelPath) {
      toast.error(hasInstalledModels ? 'Please select an installed model' : 'No installed models found. Download one first.');
      return;
    }
    if (!selectedModel.backend_ids.includes(values.backend_id)) {
      toast.error('Selected backend is not supported by this model');
      return;
    }

    try {
      await loadModelMutation.mutateAsync({
        body: {
          backend_id: values.backend_id,
          model_path: modelPath,
          num_workers: values.num_workers,
        },
      });
      toast.success('Model loaded successfully');
      setShowLoadDialog(false);
      loadForm.reset();
    } catch (error) {
      toast.error('Failed to load model');
    }
  };

  const handleSwitchModel = async (values: SwitchFormValues) => {
    const selectedModel = installedCatalogModels.find((model) => model.id === values.model_id);
    const modelPath = selectedModel?.local_path;
    if (!selectedModel || !modelPath) {
      toast.error(hasInstalledModels ? 'Please select an installed model' : 'No installed models found. Download one first.');
      return;
    }
    if (!selectedModel.backend_ids.includes(values.backend_id)) {
      toast.error('Selected backend is not supported by this model');
      return;
    }

    try {
      await switchModelMutation.mutateAsync({
        body: {
          backend_id: values.backend_id,
          model_path: modelPath,
          num_workers: values.num_workers,
        },
      });
      toast.success('Model switched successfully');
      setShowSwitchDialog(false);
      switchForm.reset();
    } catch (error) {
      toast.error('Failed to switch model');
    }
  };

  const handleUnloadModel = async () => {
    try {
      await unloadModelMutation.mutateAsync({
        body: {
          backend_id: 'ggml.llama',
          model_path: '',
        },
      });
      toast.success('Model unloaded successfully');
    } catch (error) {
      toast.error('Failed to unload model');
    }
  };

  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <Tabs defaultValue="models">
        <TabsList className="grid w-full grid-cols-2">
          <TabsTrigger value="models">Models</TabsTrigger>
          <TabsTrigger value="actions">Actions</TabsTrigger>
        </TabsList>
        
        <TabsContent value="models" className="mt-6">
          <Card>
            <CardHeader>
              <CardTitle>Model Management</CardTitle>
              <CardDescription>Download, load, switch, and unload models</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                {/* Download Model Card */}
                <Card>
                  <CardHeader className="pb-2">
                    <CardTitle className="text-lg">Download Model</CardTitle>
                    <CardDescription>Download models from HuggingFace</CardDescription>
                  </CardHeader>
                  <CardContent>
                    <Dialog open={showDownloadDialog} onOpenChange={setShowDownloadDialog}>
                      <DialogTrigger asChild>
                        <Button className="w-full" variant="outline">
                          <Download className="h-4 w-4 mr-2" />
                          Download Model
                        </Button>
                      </DialogTrigger>
                      <DialogContent className="sm:max-w-[425px]">
                        <DialogHeader>
                          <DialogTitle>Download Model</DialogTitle>
                          <DialogDescription>
                            Download a model from HuggingFace
                          </DialogDescription>
                        </DialogHeader>
                        <Form {...downloadForm}>
                          <form onSubmit={downloadForm.handleSubmit(handleDownloadModel)} className="space-y-4">
                            <FormField
                              control={downloadForm.control}
                              name="backend_id"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Backend</FormLabel>
                                  <Select onValueChange={field.onChange} defaultValue={field.value}>
                                    <FormControl>
                                      <SelectTrigger>
                                        <SelectValue placeholder="Select backend" />
                                      </SelectTrigger>
                                    </FormControl>
                                    <SelectContent>
                                      <SelectItem value="ggml.llama">LLaMA</SelectItem>
                                      <SelectItem value="ggml.whisper">Whisper</SelectItem>
                                      <SelectItem value="ggml.diffusion">Diffusion</SelectItem>
                                    </SelectContent>
                                  </Select>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                            <FormField
                              control={downloadForm.control}
                              name="model_id"
                              render={({ field }) => (
                                <FormItem>
                                  <div className="flex items-center justify-between gap-2">
                                    <FormLabel>Catalog Model</FormLabel>
                                    <Button
                                      type="button"
                                      variant="ghost"
                                      size="sm"
                                      className="h-6 px-2 text-xs"
                                      onClick={() => refetchCatalogModels()}
                                      disabled={catalogModelsLoading}
                                    >
                                      {catalogModelsLoading ? (
                                        <>
                                          <Loader2 className="h-3 w-3 mr-1 animate-spin" />
                                          Refreshing
                                        </>
                                      ) : (
                                        'Refresh'
                                      )}
                                    </Button>
                                  </div>
                                  <Select onValueChange={field.onChange} value={field.value}>
                                    <FormControl>
                                      <SelectTrigger>
                                        <SelectValue placeholder="Select model from catalog" />
                                      </SelectTrigger>
                                    </FormControl>
                                    <SelectContent>
                                      {catalogModelsLoading ? (
                                        <div className="px-2 py-1.5 text-sm text-muted-foreground">
                                          Loading catalog models...
                                        </div>
                                      ) : catalogModelsError ? (
                                        <div className="px-2 py-1.5 text-sm text-destructive">
                                          Failed to load catalog models
                                        </div>
                                      ) : compatibleCatalogModels.length === 0 ? (
                                        <div className="px-2 py-1.5 text-sm text-muted-foreground">
                                          No catalog models for this backend
                                        </div>
                                      ) : (
                                        compatibleCatalogModels.map((model) => (
                                          <SelectItem key={model.id} value={model.id}>
                                            {model.display_name}
                                          </SelectItem>
                                        ))
                                      )}
                                    </SelectContent>
                                  </Select>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                            {selectedCatalogModel ? (
                              <div className="rounded-md border p-3 space-y-1 text-xs">
                                <p className="font-medium">{selectedCatalogModel.display_name}</p>
                                <p className="text-muted-foreground">Repo: {selectedCatalogModel.repo_id}</p>
                                <p className="text-muted-foreground">File: {selectedCatalogModel.filename}</p>
                              </div>
                            ) : null}
                            <DialogFooter>
                              <Button type="button" variant="outline" onClick={() => setShowDownloadDialog(false)}>
                                Cancel
                              </Button>
                              <Button type="submit" disabled={downloadModelMutation.isPending}>
                                {downloadModelMutation.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                                Download
                              </Button>
                            </DialogFooter>
                          </form>
                        </Form>
                      </DialogContent>
                    </Dialog>
                  </CardContent>
                </Card>

                {/* Load Model Card */}
                <Card>
                  <CardHeader className="pb-2">
                    <CardTitle className="text-lg">Load Model</CardTitle>
                    <CardDescription>Load an installed model from the catalog</CardDescription>
                  </CardHeader>
                  <CardContent>
                    <Dialog
                      open={showLoadDialog}
                      onOpenChange={(open) => {
                        setShowLoadDialog(open);
                        if (open) {
                          void refetchCatalogModels();
                        }
                      }}
                    >
                      <DialogTrigger asChild>
                        <Button className="w-full" variant="outline">
                          <Upload className="h-4 w-4 mr-2" />
                          Load Model
                        </Button>
                      </DialogTrigger>
                      <DialogContent className="sm:max-w-[425px]">
                        <DialogHeader>
                          <DialogTitle>Load Model</DialogTitle>
                          <DialogDescription>
                            Load an installed catalog model
                          </DialogDescription>
                        </DialogHeader>
                        <Form {...loadForm}>
                          <form onSubmit={loadForm.handleSubmit(handleLoadModel)} className="space-y-4">
                            <FormField
                              control={loadForm.control}
                              name="model_id"
                              render={({ field }) => (
                                <FormItem>
                                  <div className="flex items-center justify-between gap-2">
                                    <FormLabel>Installed Model</FormLabel>
                                    <Button
                                      type="button"
                                      variant="ghost"
                                      size="sm"
                                      className="h-6 px-2 text-xs"
                                      onClick={() => refetchCatalogModels()}
                                      disabled={catalogModelsLoading}
                                    >
                                      {catalogModelsLoading ? (
                                        <>
                                          <Loader2 className="h-3 w-3 mr-1 animate-spin" />
                                          Refreshing
                                        </>
                                      ) : (
                                        'Refresh'
                                      )}
                                    </Button>
                                  </div>
                                  <Select onValueChange={field.onChange} value={field.value}>
                                    <FormControl>
                                      <SelectTrigger>
                                        <SelectValue placeholder="Select installed model" />
                                      </SelectTrigger>
                                    </FormControl>
                                    <SelectContent>
                                      {catalogModelsLoading ? (
                                        <div className="px-2 py-1.5 text-sm text-muted-foreground">
                                          Loading catalog models...
                                        </div>
                                      ) : catalogModelsError ? (
                                        <div className="px-2 py-1.5 text-sm text-destructive">
                                          Failed to load catalog models
                                        </div>
                                      ) : installedCatalogModels.length === 0 ? (
                                        <div className="px-2 py-1.5 text-sm text-muted-foreground">
                                          No installed models found
                                        </div>
                                      ) : (
                                        installedCatalogModels.map((model) => (
                                          <SelectItem key={model.id} value={model.id}>
                                            {model.display_name}
                                          </SelectItem>
                                        ))
                                      )}
                                    </SelectContent>
                                  </Select>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                            <FormField
                              control={loadForm.control}
                              name="backend_id"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Backend</FormLabel>
                                  <Select
                                    onValueChange={field.onChange}
                                    value={field.value}
                                    disabled={!selectedLoadModel}
                                  >
                                    <FormControl>
                                      <SelectTrigger>
                                        <SelectValue placeholder="Select backend" />
                                      </SelectTrigger>
                                    </FormControl>
                                    <SelectContent>
                                      {selectedLoadModel ? (
                                        selectedLoadModel.backend_ids.map((backendId) => (
                                          <SelectItem key={backendId} value={backendId}>
                                            {backendId}
                                          </SelectItem>
                                        ))
                                      ) : (
                                        <div className="px-2 py-1.5 text-sm text-muted-foreground">
                                          Select an installed model first
                                        </div>
                                      )}
                                    </SelectContent>
                                  </Select>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                            {selectedLoadModel?.local_path ? (
                              <div className="rounded-md border p-3 space-y-1 text-xs">
                                <p className="font-medium">{selectedLoadModel.display_name}</p>
                                <p className="text-muted-foreground">Path: {selectedLoadModel.local_path}</p>
                              </div>
                            ) : null}
                            {!catalogModelsLoading && !catalogModelsError && !hasInstalledModels ? (
                              <div className="rounded-md border border-amber-300 bg-amber-50 p-3 space-y-2 text-xs">
                                <p className="text-amber-900">No installed models found. Download a model first.</p>
                                <Button
                                  type="button"
                                  size="sm"
                                  variant="outline"
                                  className="h-7"
                                  onClick={openDownloadDialog}
                                >
                                  Open Download Model
                                </Button>
                              </div>
                            ) : null}
                            <FormField
                              control={loadForm.control}
                              name="num_workers"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Number of Workers</FormLabel>
                                  <FormControl>
                                    <Input type="number" min="1" {...field} />
                                  </FormControl>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                            <DialogFooter>
                              <Button type="button" variant="outline" onClick={() => setShowLoadDialog(false)}>
                                Cancel
                              </Button>
                              <Button type="submit" disabled={loadModelMutation.isPending}>
                                {loadModelMutation.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                                Load
                              </Button>
                            </DialogFooter>
                          </form>
                        </Form>
                      </DialogContent>
                    </Dialog>
                  </CardContent>
                </Card>

                {/* Switch Model Card */}
                <Card>
                  <CardHeader className="pb-2">
                    <CardTitle className="text-lg">Switch Model</CardTitle>
                    <CardDescription>Switch between installed catalog models</CardDescription>
                  </CardHeader>
                  <CardContent>
                    <Dialog
                      open={showSwitchDialog}
                      onOpenChange={(open) => {
                        setShowSwitchDialog(open);
                        if (open) {
                          void refetchCatalogModels();
                        }
                      }}
                    >
                      <DialogTrigger asChild>
                        <Button className="w-full" variant="outline">
                          <RefreshCw className="h-4 w-4 mr-2" />
                          Switch Model
                        </Button>
                      </DialogTrigger>
                      <DialogContent className="sm:max-w-[425px]">
                        <DialogHeader>
                          <DialogTitle>Switch Model</DialogTitle>
                          <DialogDescription>
                            Switch between installed catalog models
                          </DialogDescription>
                        </DialogHeader>
                        <Form {...switchForm}>
                          <form onSubmit={switchForm.handleSubmit(handleSwitchModel)} className="space-y-4">
                            <FormField
                              control={switchForm.control}
                              name="model_id"
                              render={({ field }) => (
                                <FormItem>
                                  <div className="flex items-center justify-between gap-2">
                                    <FormLabel>Installed Model</FormLabel>
                                    <Button
                                      type="button"
                                      variant="ghost"
                                      size="sm"
                                      className="h-6 px-2 text-xs"
                                      onClick={() => refetchCatalogModels()}
                                      disabled={catalogModelsLoading}
                                    >
                                      {catalogModelsLoading ? (
                                        <>
                                          <Loader2 className="h-3 w-3 mr-1 animate-spin" />
                                          Refreshing
                                        </>
                                      ) : (
                                        'Refresh'
                                      )}
                                    </Button>
                                  </div>
                                  <Select onValueChange={field.onChange} value={field.value}>
                                    <FormControl>
                                      <SelectTrigger>
                                        <SelectValue placeholder="Select installed model" />
                                      </SelectTrigger>
                                    </FormControl>
                                    <SelectContent>
                                      {catalogModelsLoading ? (
                                        <div className="px-2 py-1.5 text-sm text-muted-foreground">
                                          Loading catalog models...
                                        </div>
                                      ) : catalogModelsError ? (
                                        <div className="px-2 py-1.5 text-sm text-destructive">
                                          Failed to load catalog models
                                        </div>
                                      ) : installedCatalogModels.length === 0 ? (
                                        <div className="px-2 py-1.5 text-sm text-muted-foreground">
                                          No installed models found
                                        </div>
                                      ) : (
                                        installedCatalogModels.map((model) => (
                                          <SelectItem key={model.id} value={model.id}>
                                            {model.display_name}
                                          </SelectItem>
                                        ))
                                      )}
                                    </SelectContent>
                                  </Select>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                            <FormField
                              control={switchForm.control}
                              name="backend_id"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Backend</FormLabel>
                                  <Select
                                    onValueChange={field.onChange}
                                    value={field.value}
                                    disabled={!selectedSwitchModel}
                                  >
                                    <FormControl>
                                      <SelectTrigger>
                                        <SelectValue placeholder="Select backend" />
                                      </SelectTrigger>
                                    </FormControl>
                                    <SelectContent>
                                      {selectedSwitchModel ? (
                                        selectedSwitchModel.backend_ids.map((backendId) => (
                                          <SelectItem key={backendId} value={backendId}>
                                            {backendId}
                                          </SelectItem>
                                        ))
                                      ) : (
                                        <div className="px-2 py-1.5 text-sm text-muted-foreground">
                                          Select an installed model first
                                        </div>
                                      )}
                                    </SelectContent>
                                  </Select>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                            {selectedSwitchModel?.local_path ? (
                              <div className="rounded-md border p-3 space-y-1 text-xs">
                                <p className="font-medium">{selectedSwitchModel.display_name}</p>
                                <p className="text-muted-foreground">Path: {selectedSwitchModel.local_path}</p>
                              </div>
                            ) : null}
                            {!catalogModelsLoading && !catalogModelsError && !hasInstalledModels ? (
                              <div className="rounded-md border border-amber-300 bg-amber-50 p-3 space-y-2 text-xs">
                                <p className="text-amber-900">No installed models found. Download a model first.</p>
                                <Button
                                  type="button"
                                  size="sm"
                                  variant="outline"
                                  className="h-7"
                                  onClick={openDownloadDialog}
                                >
                                  Open Download Model
                                </Button>
                              </div>
                            ) : null}
                            <FormField
                              control={switchForm.control}
                              name="num_workers"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Number of Workers</FormLabel>
                                  <FormControl>
                                    <Input type="number" min="1" {...field} />
                                  </FormControl>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                            <DialogFooter>
                              <Button type="button" variant="outline" onClick={() => setShowSwitchDialog(false)}>
                                Cancel
                              </Button>
                              <Button type="submit" disabled={switchModelMutation.isPending}>
                                {switchModelMutation.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                                Switch
                              </Button>
                            </DialogFooter>
                          </form>
                        </Form>
                      </DialogContent>
                    </Dialog>
                  </CardContent>
                </Card>

                {/* Unload Model Card */}
                <Card>
                  <CardHeader className="pb-2">
                    <CardTitle className="text-lg">Unload Model</CardTitle>
                    <CardDescription>Unload the currently loaded model</CardDescription>
                  </CardHeader>
                  <CardContent>
                    <Button 
                      className="w-full" 
                      variant="destructive" 
                      onClick={handleUnloadModel}
                      disabled={unloadModelMutation.isPending}
                    >
                      {unloadModelMutation.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                      <X className="h-4 w-4 mr-2" />
                      Unload Model
                    </Button>
                  </CardContent>
                </Card>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
        
        <TabsContent value="actions" className="mt-6">
          <Card>
            <CardHeader>
              <CardTitle>Recent Actions</CardTitle>
              <CardDescription>View recent model operations</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="flex items-center justify-center py-12">
                <div className="text-center space-y-2">
                  <p className="text-muted-foreground text-sm">No recent actions</p>
                  <p className="text-muted-foreground text-xs">Model operations will appear here once performed</p>
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
