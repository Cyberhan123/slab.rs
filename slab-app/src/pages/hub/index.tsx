import { useState } from 'react';
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
  repo_id: z.string().min(1, 'Repository ID is required'),
  filename: z.string().min(1, 'Filename is required'),
  target_dir: z.string().optional(),
});

const loadSchema = z.object({
  backend_id: z.string().min(1, 'Backend ID is required'),
  model_path: z.string().min(1, 'Model path is required'),
  num_workers: z.number().optional(),
});

const switchSchema = z.object({
  backend_id: z.string().min(1, 'Backend ID is required'),
  model_path: z.string().min(1, 'Model path is required'),
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
  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const loadModelMutation = api.useMutation('post', '/v1/models/load');
  const switchModelMutation = api.useMutation('post', '/v1/models/switch');
  const unloadModelMutation = api.useMutation('post', '/v1/models/unload');

  // Forms
  const downloadForm = useForm<DownloadFormValues>({
    resolver: zodResolver(downloadSchema),
    defaultValues: {
      backend_id: 'ggml.llama',
      repo_id: '',
      filename: '',
      target_dir: '',
    },
  });

  const loadForm = useForm<LoadFormValues>({
    resolver: zodResolver(loadSchema),
    defaultValues: {
      backend_id: 'ggml.llama',
      model_path: '',
      num_workers: 1,
    },
  });

  const switchForm = useForm<SwitchFormValues>({
    resolver: zodResolver(switchSchema),
    defaultValues: {
      backend_id: 'ggml.llama',
      model_path: '',
      num_workers: 1,
    },
  });

  // Handlers
  const handleDownloadModel = async (values: DownloadFormValues) => {
    try {
      await downloadModelMutation.mutateAsync({
        body: values,
      });
      toast.success('Model download initiated');
      setShowDownloadDialog(false);
      downloadForm.reset();
    } catch (error) {
      toast.error('Failed to download model');
    }
  };

  const handleLoadModel = async (values: LoadFormValues) => {
    try {
      await loadModelMutation.mutateAsync({
        body: values,
      });
      toast.success('Model loaded successfully');
      setShowLoadDialog(false);
      loadForm.reset();
    } catch (error) {
      toast.error('Failed to load model');
    }
  };

  const handleSwitchModel = async (values: SwitchFormValues) => {
    try {
      await switchModelMutation.mutateAsync({
        body: values,
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
                              name="repo_id"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Repository ID</FormLabel>
                                  <FormControl>
                                    <Input placeholder="e.g., bartowski/Qwen2.5-0.5B-Instruct-GGUF" {...field} />
                                  </FormControl>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                            <FormField
                              control={downloadForm.control}
                              name="filename"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Filename</FormLabel>
                                  <FormControl>
                                    <Input placeholder="e.g., Qwen2.5-0.5B-Instruct-Q4_K_M.gguf" {...field} />
                                  </FormControl>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                            <FormField
                              control={downloadForm.control}
                              name="target_dir"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Target Directory (Optional)</FormLabel>
                                  <FormControl>
                                    <Input placeholder="Leave empty for default cache" {...field} />
                                  </FormControl>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
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
                    <CardDescription>Load a model from local storage</CardDescription>
                  </CardHeader>
                  <CardContent>
                    <Dialog open={showLoadDialog} onOpenChange={setShowLoadDialog}>
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
                            Load a model from local storage
                          </DialogDescription>
                        </DialogHeader>
                        <Form {...loadForm}>
                          <form onSubmit={loadForm.handleSubmit(handleLoadModel)} className="space-y-4">
                            <FormField
                              control={loadForm.control}
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
                              control={loadForm.control}
                              name="model_path"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Model Path</FormLabel>
                                  <FormControl>
                                    <Input placeholder="Path to model file" {...field} />
                                  </FormControl>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
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
                    <CardDescription>Switch to a different model</CardDescription>
                  </CardHeader>
                  <CardContent>
                    <Dialog open={showSwitchDialog} onOpenChange={setShowSwitchDialog}>
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
                            Switch to a different model
                          </DialogDescription>
                        </DialogHeader>
                        <Form {...switchForm}>
                          <form onSubmit={switchForm.handleSubmit(handleSwitchModel)} className="space-y-4">
                            <FormField
                              control={switchForm.control}
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
                              control={switchForm.control}
                              name="model_path"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Model Path</FormLabel>
                                  <FormControl>
                                    <Input placeholder="Path to model file" {...field} />
                                  </FormControl>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
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
