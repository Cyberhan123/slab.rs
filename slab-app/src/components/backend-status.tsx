import { useEffect, useState } from 'react';
import { Badge } from '@/components/ui/badge';
import { checkBackendStatus } from '@/lib/tauri-api';

export function BackendStatus() {
  const [isOnline, setIsOnline] = useState<boolean | null>(null);
  const [isChecking, setIsChecking] = useState(true);

  useEffect(() => {
    const checkStatus = async () => {
      setIsChecking(true);
      const status = await checkBackendStatus();
      setIsOnline(status);
      setIsChecking(false);
    };

    // Check immediately
    checkStatus();

    // Check every 30 seconds
    const interval = setInterval(checkStatus, 30000);

    return () => clearInterval(interval);
  }, []);

  if (isChecking) {
    return (
      <Badge variant="outline" className="gap-1">
        <div className="h-2 w-2 rounded-full bg-yellow-500 animate-pulse" />
        Checking...
      </Badge>
    );
  }

  if (isOnline === null) {
    return (
      <Badge variant="outline" className="gap-1">
        <div className="h-2 w-2 rounded-full bg-gray-500" />
        Unknown
      </Badge>
    );
  }

  if (isOnline) {
    return (
      <Badge variant="default" className="gap-1">
        <div className="h-2 w-2 rounded-full bg-green-500" />
        Online
      </Badge>
    );
  }

  return (
    <Badge variant="destructive" className="gap-1">
      <div className="h-2 w-2 rounded-full bg-red-500" />
      Offline
    </Badge>
  );
}
