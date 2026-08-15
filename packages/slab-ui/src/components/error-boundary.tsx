import React from 'react';
import { Button } from '@slab/components/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@slab/components/card';
import { AlertCircle, ArrowLeft, RefreshCw } from 'lucide-react';
import { useTranslation } from '@slab/i18n';
import { useNavigate } from 'react-router-dom';

type ErrorBoundaryVariant = 'global' | 'page';

export type ErrorBoundaryFallbackProps = {
  error: Error;
  retry: () => void;
  variant: ErrorBoundaryVariant;
};

interface Props {
  children: React.ReactNode;
  fallback?: React.ComponentType<ErrorBoundaryFallbackProps>;
  variant?: ErrorBoundaryVariant;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
  }

  retry = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      const { fallback: Fallback } = this.props;

      if (Fallback) {
        return (
          <Fallback
            error={this.state.error!}
            retry={this.retry}
            variant={this.props.variant ?? 'global'}
          />
        );
      }

      return (
        <DefaultErrorFallback
          error={this.state.error!}
          retry={this.retry}
          variant={this.props.variant ?? 'global'}
        />
      );
    }

    return this.props.children;
  }
}

function DefaultErrorFallback(props: ErrorBoundaryFallbackProps) {
  if (props.variant === 'page') {
    return <PageErrorFallback {...props} />;
  }

  return <GlobalErrorFallback {...props} />;
}

function GlobalErrorFallback({ error, retry }: ErrorBoundaryFallbackProps) {
  const { t } = useTranslation();

  return (
    <div className="flex min-h-screen items-center justify-center bg-app-canvas p-4 text-foreground">
      <ErrorCard
        action={
          <Button onClick={retry} className="w-full">
            <RefreshCw className="mr-2 h-4 w-4" />
            {t('layouts.errorBoundary.global.retry')}
          </Button>
        }
        description={t('layouts.errorBoundary.global.description')}
        error={error}
        title={t('layouts.errorBoundary.global.title')}
      />
    </div>
  );
}

function PageErrorFallback({ error, retry }: ErrorBoundaryFallbackProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();

  function goBack() {
    if (window.history.length > 1) {
      navigate(-1);
      return;
    }

    navigate('/', { replace: true });
  }

  return (
    <div className="flex min-h-0 flex-1 items-center justify-center p-4 text-foreground">
      <ErrorCard
        action={
          <div className="flex flex-col gap-2 sm:flex-row">
            <Button onClick={goBack} className="w-full sm:w-auto">
              <ArrowLeft className="mr-2 h-4 w-4" />
              {t('layouts.errorBoundary.page.back')}
            </Button>
            <Button variant="quiet" onClick={retry} className="w-full sm:w-auto">
              <RefreshCw className="mr-2 h-4 w-4" />
              {t('layouts.errorBoundary.page.retry')}
            </Button>
          </div>
        }
        description={t('layouts.errorBoundary.page.description')}
        error={error}
        title={t('layouts.errorBoundary.page.title')}
      />
    </div>
  );
}

function ErrorCard({
  action,
  description,
  error,
  title,
}: {
  action: React.ReactNode;
  description: string;
  error: Error;
  title: string;
}) {
  const { t } = useTranslation();

  return (
    <Card className="w-full max-w-lg">
      <CardHeader>
        <div className="flex items-center gap-2">
          <AlertCircle className="h-6 w-6 text-destructive" />
          <CardTitle>{title}</CardTitle>
        </div>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <details className="text-sm text-muted-foreground">
          <summary className="cursor-pointer">{t('layouts.errorBoundary.details')}</summary>
          <pre className="mt-2 overflow-auto rounded bg-muted p-2 text-xs">
            {error.toString()}
          </pre>
        </details>
        {action}
      </CardContent>
    </Card>
  );
}

// Hook version for functional components
export function useErrorHandler() {
  return React.useCallback((error: Error) => {
    throw error;
  }, []);
}
