import { AlertCircle } from 'lucide-react';

import { getErrorData, type AppCoreErrorData } from '@slab/api';

type ErrorDataDetailProps = {
  data?: AppCoreErrorData | null;
  error?: unknown;
};

export function ErrorDataDetail({ data, error }: ErrorDataDetailProps) {
  const detail = data ?? getErrorData<AppCoreErrorData>(error);
  if (!detail) {
    return null;
  }

  return (
    <div className="rounded-[10px] border border-border/60 bg-[var(--surface-soft)] px-3 py-2 text-[11px] leading-4 text-muted-foreground">
      <div className="flex items-center gap-1.5 font-semibold text-foreground">
        <AlertCircle className="size-3.5" />
        {detail.code}
      </div>
      <p className="mt-1 break-words">{describeErrorData(detail)}</p>
    </div>
  );
}

function describeErrorData(data: AppCoreErrorData): string {
  switch (data.code) {
    case 'unsupported_chat_parameter': {
      const detail = data as Extract<AppCoreErrorData, { code: 'unsupported_chat_parameter' }>;
      return `Unsupported parameter: ${detail.param}`;
    }
    case 'model_download_unavailable': {
      const detail = data as Extract<AppCoreErrorData, { code: 'model_download_unavailable' }>;
      return `${detail.reason}${detail.suggestion ? `. ${detail.suggestion}` : ''}`;
    }
    case 'runtime_failure': {
      const detail = data as Extract<AppCoreErrorData, { code: 'runtime_failure' }>;
      return detail.runtime_code;
    }
    default:
      return JSON.stringify(data) ?? String(data.code);
  }
}
