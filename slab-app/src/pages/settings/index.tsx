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
import { toast } from 'sonner';
import { Loader2 } from 'lucide-react';

export default function Settings() {
  const [selectedConfigKey, setSelectedConfigKey] = useState<string | null>(null);
  const [configValue, setConfigValue] = useState<string>('');

  // API calls using react-query
  const { data: configs, error: configsError, isLoading: configsLoading, refetch: refetchConfigs } = api.useQuery('get', '/admin/config');
  const { data: backends, error: backendsError, isLoading: backendsLoading } = api.useQuery('get', '/admin/backends');
  // Mutation for updating config
  const updateConfigMutation = api.useMutation('put', '/admin/config/{key}');
  
  // Mutation for getting backend status
  const getBackendStatusMutation = api.useMutation('get', '/admin/backends/status');
  
  // Mutation for downloading backend
  const downloadBackendMutation = api.useMutation('get', '/admin/backends/download');
  
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
      toast.error('Failed to update configuration');
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
      toast.error('Failed to get backend status');
    }
  };

  // Function to download backend
  const downloadBackend = async () => {
    try {
      await downloadBackendMutation.mutateAsync({
        body: {
          target_path: '', // Provide a default target path
        },
      });
      toast.success('Backend download initiated');
    } catch (error) {
      toast.error('Failed to download backend');
    }
  };

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
                    {(backends?.backends?.length??0) > 0 ? (
                      backends.backends.map((backend: any) => (
                        <TableRow key={backend.backend}>
                          <TableCell className="font-medium">{backend.backend}</TableCell>
                          <TableCell>
                            <span className={`px-2 py-1 text-xs rounded-full ${backend.status === 'running' ? 'bg-green-100 text-green-800' : 'bg-yellow-100 text-yellow-800'}`}>
                              {backend.status}
                            </span>
                          </TableCell>
                          <TableCell className="flex gap-2">
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => getBackendStatus(backend.backend)}
                            >
                              Check Status
                            </Button>
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={downloadBackend}
                            >
                              Download
                            </Button>
                          </TableCell>
                        </TableRow>
                      ))
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