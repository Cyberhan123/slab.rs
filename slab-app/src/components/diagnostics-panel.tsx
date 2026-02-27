/**
 * Diagnostics Panel Component
 *
 * Displays API diagnostic information and allows management of diagnostic logs
 */

import { useState, useEffect } from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui/scroll-area';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { toast } from 'sonner';
import {
  getDiagnosticSummary,
  getLogs,
  getLogsByType,
  getLogsByLevel,
  clearLogs,
  exportLogs,
  createDiagnosticReport,
  testApiConnectivity,
  verifyApiConfig,
  type DiagnosticEntry,
  type LogLevel,
  initDiagnostics,
} from '@/lib/api/diagnostics';
import { RefreshCw, Download, Trash2, CheckCircle, XCircle, AlertCircle } from 'lucide-react';

export function DiagnosticsPanel() {
  const [logs, setLogs] = useState<DiagnosticEntry[]>([]);
  const [filter, setFilter] = useState<'all' | LogLevel | 'request' | 'response' | 'error' | 'health'>('all');
  const [summary, setSummary] = useState(getDiagnosticSummary());
  const [healthStatus, setHealthStatus] = useState<Awaited<ReturnType<typeof testApiConnectivity>> | null>(null);
  const [configStatus, setConfigStatus] = useState(verifyApiConfig());
  const [isRefreshing, setIsRefreshing] = useState(false);

  useEffect(() => {
    refreshLogs();
    // Auto-refresh every 5 seconds
    const interval = setInterval(refreshLogs, 5000);
    return () => clearInterval(interval);
  }, [filter]);

  const refreshLogs = () => {
    let filteredLogs: DiagnosticEntry[];

    switch (filter) {
      case 'all':
        filteredLogs = getLogs();
        break;
      case 'debug':
      case 'info':
      case 'warn':
      case 'error':
        filteredLogs = getLogsByLevel(filter as LogLevel);
        break;
      case 'request':
      case 'response':
      case 'health':
        filteredLogs = getLogsByType(filter);
        break;
      default:
        filteredLogs = getLogs();
    }

    // Show last 50 logs
    setLogs(filteredLogs.slice(-50).reverse());
    setSummary(getDiagnosticSummary());
  };

  const handleTestConnectivity = async () => {
    setIsRefreshing(true);
    const result = await testApiConnectivity();
    setHealthStatus(result);
    setIsRefreshing(false);

    if (result.success) {
      toast.success('Backend is reachable', {
        description: `Response time: ${result.duration}ms`,
      });
    } else {
      toast.error('Backend is unreachable', {
        description: result.error || `Status: ${result.status}`,
      });
    }
  };

  const handleVerifyConfig = () => {
    const result = verifyApiConfig();
    setConfigStatus(result);
    refreshLogs();

    if (result.correct) {
      toast.success('API configuration is valid');
    } else {
      toast.warning('API configuration has issues', {
        description: result.issues.join(', '),
      });
    }
  };

  const handleClearLogs = () => {
    clearLogs();
    refreshLogs();
    toast.success('Diagnostic logs cleared');
  };

  const handleExportLogs = () => {
    const report = createDiagnosticReport();
    const blob = new Blob([report], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `slab-diagnostics-${Date.now()}.txt`;
    a.click();
    URL.revokeObjectURL(url);
    toast.success('Diagnostic report exported');
  };

  const getLevelIcon = (level: LogLevel) => {
    switch (level) {
      case 'error':
        return <XCircle className="h-4 w-4 text-destructive" />;
      case 'warn':
        return <AlertCircle className="h-4 w-4 text-yellow-500" />;
      case 'info':
        return <CheckCircle className="h-4 w-4 text-blue-500" />;
      case 'debug':
        return <CheckCircle className="h-4 w-4 text-gray-500" />;
    }
  };

  const getTypeBadge = (type: DiagnosticEntry['type']) => {
    const variants: Record<DiagnosticEntry['type'], 'default' | 'secondary' | 'destructive' | 'outline'> = {
      request: 'secondary',
      response: 'default',
      error: 'destructive',
      health: 'outline',
    };

    return <Badge variant={variants[type]}>{type}</Badge>;
  };

  return (
    <div className="container mx-auto p-4 space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">API Diagnostics</h1>
          <p className="text-sm text-muted-foreground">
            Monitor API requests, responses, and connectivity
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={handleTestConnectivity}
            disabled={isRefreshing}
          >
            <RefreshCw className={`h-4 w-4 mr-2 ${isRefreshing ? 'animate-spin' : ''}`} />
            Test Connectivity
          </Button>
          <Button variant="outline" size="sm" onClick={handleVerifyConfig}>
            Verify Config
          </Button>
          <Button variant="outline" size="sm" onClick={handleExportLogs}>
            <Download className="h-4 w-4 mr-2" />
            Export
          </Button>
          <Button variant="outline" size="sm" onClick={handleClearLogs}>
            <Trash2 className="h-4 w-4 mr-2" />
            Clear
          </Button>
        </div>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Total Logs</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{summary.totalLogs}</div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Errors</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-destructive">
              {summary.logsByLevel.error}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">API Mode</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{summary.apiConfig.mode}</div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Backend Status</CardTitle>
          </CardHeader>
          <CardContent>
            {healthStatus ? (
              <div className="flex items-center gap-2">
                {healthStatus.success ? (
                  <CheckCircle className="h-5 w-5 text-green-500" />
                ) : (
                  <XCircle className="h-5 w-5 text-destructive" />
                )}
                <span className="text-sm">
                  {healthStatus.success ? 'Online' : 'Offline'}
                </span>
              </div>
            ) : (
              <Button size="sm" variant="outline" onClick={handleTestConnectivity}>
                Check
              </Button>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Configuration Status */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">API Configuration</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-muted-foreground">Base URL:</span>
              <span className="font-mono">{summary.apiConfig.baseUrl}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-muted-foreground">Valid:</span>
              <span>{configStatus.correct ? 'Yes' : 'No'}</span>
            </div>
            {!configStatus.correct && configStatus.issues.length > 0 && (
              <div className="text-destructive">
                Issues: {configStatus.issues.join(', ')}
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Logs */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="text-base">Diagnostic Logs</CardTitle>
              <CardDescription>Recent API activity</CardDescription>
            </div>
            <Select value={filter} onValueChange={(v) => setFilter(v as typeof filter)}>
              <SelectTrigger className="w-40">
                <SelectValue placeholder="Filter logs" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Logs</SelectItem>
                <SelectItem value="request">Requests</SelectItem>
                <SelectItem value="response">Responses</SelectItem>
                <SelectItem value="error">Errors</SelectItem>
                <SelectItem value="health">Health</SelectItem>
                <SelectItem value="debug">Debug</SelectItem>
                <SelectItem value="info">Info</SelectItem>
                <SelectItem value="warn">Warnings</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardHeader>
        <CardContent>
          <ScrollArea className="h-96">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-24">Time</TableHead>
                  <TableHead className="w-20">Level</TableHead>
                  <TableHead className="w-24">Type</TableHead>
                  <TableHead>Data</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {logs.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={4} className="text-center text-muted-foreground">
                      No logs to display
                    </TableCell>
                  </TableRow>
                ) : (
                  logs.map((log, index) => (
                    <TableRow key={index}>
                      <TableCell className="text-xs font-mono">
                        {new Date(log.timestamp).toLocaleTimeString()}
                      </TableCell>
                      <TableCell>{getLevelIcon(log.level)}</TableCell>
                      <TableCell>{getTypeBadge(log.type)}</TableCell>
                      <TableCell className="text-xs font-mono max-w-md truncate">
                        {JSON.stringify(log.data, null, 2)}
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </ScrollArea>
        </CardContent>
      </Card>
    </div>
  );
}
