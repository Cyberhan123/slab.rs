import { useState } from 'react';
import api, { getErrorMessage } from "@/lib/api";
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
  FormControl,
  FormItem,
  FormLabel,
} from '@/components/ui/form';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { toast } from 'sonner';
import { Loader2, CheckCircle2, XCircle, AlertCircle } from 'lucide-react';

interface BackendListItem {
  model_type: string;
  backend: string;
  status: string;
}

export default function Settings() {
  const [selectedConfigKey, setSelectedConfigKey] = useState<string | null>(null);
  const [configValue, setConfigValue] = useState<string>('');
  const [downloadingBackend, setDownloadingBackend] = useState<string | null>(null);

  // API calls using react-query
  const { data: configs, error: configsError, isLoading: configsLoading, refetch: refetchConfigs } = api.useQuery('get', '/admin/config');
  const { data: backends, error: backendsError, isLoading: backendsLoading } = api.useQuery('get', '/admin/backends');
  // Mutation for updating config
  const updateConfigMutation = api.useMutation('put', '/admin/config/{key}');
  
  // Mutation for getting backend status
  const getBackendStatusMutation = api.useMutation('get', '/admin/backends/status');
  
  // Function to update config value
  const updateConfig = async (key: string, value: string) => {
    try {
      await updateConfigMutation.mutateAsync({
        params: {
          path: { key },
        },
        body: {
          value,
        },
      });
      toast.success('Configuration updated successfully');
      // Refresh configs
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

  const backendList =
    typeof backends === 'object' &&
    backends !== null &&
    Array.isArray((backends as { backends?: unknown }).backends)
      ? ((backends as { backends: BackendListItem[] }).backends ?? [])
      : [];

  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <h1 className="text-3xl font-bold">Settings</h1>
      
      <Tabs defaultValue="config">
        <TabsList className="grid w-full grid-cols-2">
          <TabsTrigger value="config">Configuration</TabsTrigger>
          <TabsTrigger value="backends">Backends</TabsTrigger>
        </TabsList>
        
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
                      <TableHead>Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {(configs?.length ?? 0) > 0 ? (
                      configs?.map((config) => (
                        <TableRow key={config.key}>
                          <TableCell className="font-medium">{config.key}</TableCell>
                          <TableCell>{config.value}</TableCell>
                          <TableCell>
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => {
                                setSelectedConfigKey(config.key);
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
                        <TableCell colSpan={3} className="text-center py-4">
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
                        updateConfig(selectedConfigKey!, configValue);
                        setSelectedConfigKey(null);
                      }}
                      className="space-y-4"
                    >
                      <FormItem>
                        <FormLabel>Value</FormLabel>
                        <FormControl>
                          <Input
                            value={configValue}
                            onChange={(e) => setConfigValue(e.target.value)}
                            placeholder="Enter new value"
                          />
                        </FormControl>
                      </FormItem>
                      <div className="flex gap-2">
                        <Button type="submit">Save</Button>
                        <Button
                          variant="outline"
                          onClick={() => setSelectedConfigKey(null)}
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
    </div>
  );
}
