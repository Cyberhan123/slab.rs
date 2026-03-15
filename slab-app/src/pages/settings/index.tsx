import { useDeferredValue, useMemo, useState } from 'react';
import {
  Loader2,
  RefreshCw,
  RotateCcw,
  Save,
  Search,
  Settings2,
  TriangleAlert,
} from 'lucide-react';
import { toast } from 'sonner';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Separator } from '@/components/ui/separator';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import api, { getErrorMessage, isApiError, paths } from '@/lib/api';
import { cn } from '@/lib/utils';

type SettingsDocumentResponse =
  paths['/v1/settings']['get']['responses'][200]['content']['application/json'];
type SettingsSectionResponse = SettingsDocumentResponse['sections'][number];
type SettingsSubsectionResponse = SettingsSectionResponse['subsections'][number];
type SettingResponse =
  paths['/v1/settings/{pmid}']['get']['responses'][200]['content']['application/json'];
type UpdateSettingRequest =
  paths['/v1/settings/{pmid}']['put']['requestBody']['content']['application/json'];
type FieldErrorState = {
  message: string;
  path: string;
};
type DraftValue = boolean | string;
type BusyState = {
  op: 'save' | 'reset';
  pmid: string;
} | null;
type SettingValidationErrorData = {
  message?: unknown;
  path?: unknown;
  pmid?: unknown;
  type?: unknown;
};

function countProperties(sections: SettingsSectionResponse[]): number {
  return sections.reduce(
    (sectionTotal, section) =>
      sectionTotal +
      section.subsections.reduce(
        (subsectionTotal, subsection) => subsectionTotal + subsection.properties.length,
        0,
      ),
    0,
  );
}

function matchesSearch(
  section: SettingsSectionResponse,
  subsection: SettingsSubsectionResponse,
  property: SettingResponse,
  query: string,
): boolean {
  if (!query) {
    return true;
  }

  const haystack = [
    section.title,
    section.description_md,
    subsection.title,
    subsection.description_md,
    property.pmid,
    property.label,
    property.description_md,
    ...property.search_terms,
  ]
    .join(' ')
    .toLowerCase();

  return haystack.includes(query);
}

function valueToEditorString(value: unknown): string {
  if (typeof value === 'string') {
    return value;
  }

  if (typeof value === 'number') {
    return Number.isFinite(value) ? String(value) : '';
  }

  if (value === null || value === undefined) {
    return '';
  }

  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return '';
  }
}

function summarizeValue(value: unknown): string {
  if (typeof value === 'string') {
    return value.length === 0 ? '(empty string)' : value;
  }

  if (
    typeof value === 'number' ||
    typeof value === 'boolean' ||
    value === null ||
    value === undefined
  ) {
    return String(value ?? 'null');
  }

  const text = valueToEditorString(value);
  return text.length > 80 ? `${text.slice(0, 77)}...` : text;
}

function extractStructuredError(error: unknown): FieldErrorState | null {
  if (!isApiError(error) || typeof error.data !== 'object' || error.data === null) {
    return null;
  }

  const data = error.data as SettingValidationErrorData;
  if (typeof data.message !== 'string' || typeof data.path !== 'string') {
    return null;
  }

  return {
    message: data.message,
    path: data.path,
  };
}

function buildRequestBody(
  property: SettingResponse,
  draftValue: DraftValue | undefined,
): UpdateSettingRequest {
  const propertyType = property.schema.type;

  if (propertyType === 'boolean') {
    return {
      op: 'set',
      value:
        typeof draftValue === 'boolean'
          ? draftValue
          : Boolean(property.effective_value),
    };
  }

  const rawValue =
    typeof draftValue === 'string'
      ? draftValue
      : valueToEditorString(property.effective_value);

  if (propertyType === 'integer') {
    const trimmed = rawValue.trim();
    if (!trimmed) {
      return { op: 'unset' };
    }
    if (!/^-?\d+$/.test(trimmed)) {
      throw new Error('Value must be an integer.');
    }
    return {
      op: 'set',
      value: Number.parseInt(trimmed, 10),
    };
  }

  if (propertyType === 'array' || propertyType === 'object') {
    const trimmed = rawValue.trim();
    if (!trimmed) {
      return { op: 'unset' };
    }
    try {
      return {
        op: 'set',
        value: JSON.parse(trimmed),
      };
    } catch {
      throw new Error('Value must be valid JSON.');
    }
  }

  return {
    op: 'set',
    value: rawValue,
  };
}

export default function SettingsPage() {
  const [search, setSearch] = useState('');
  const [drafts, setDrafts] = useState<Record<string, DraftValue>>({});
  const [fieldErrors, setFieldErrors] = useState<Record<string, FieldErrorState>>({});
  const [busy, setBusy] = useState<BusyState>(null);

  const deferredSearch = useDeferredValue(search);
  const normalizedSearch = deferredSearch.trim().toLowerCase();

  const {
    data,
    error,
    isLoading,
    isRefetching,
    refetch,
  } = api.useQuery('get', '/v1/settings');
  const updateSettingMutation = api.useMutation('put', '/v1/settings/{pmid}');

  const filteredSections = useMemo(() => {
    if (!data) {
      return [];
    }

    return data.sections
      .map((section) => ({
        ...section,
        subsections: section.subsections
          .map((subsection) => ({
            ...subsection,
            properties: subsection.properties.filter((property) =>
              matchesSearch(section, subsection, property, normalizedSearch),
            ),
          }))
          .filter((subsection) => subsection.properties.length > 0),
      }))
      .filter((section) => section.subsections.length > 0);
  }, [data, normalizedSearch]);

  const totalPropertyCount = useMemo(
    () => countProperties(data?.sections ?? []),
    [data],
  );
  const visiblePropertyCount = useMemo(
    () => countProperties(filteredSections),
    [filteredSections],
  );

  function clearFieldError(pmid: string) {
    setFieldErrors((current) => {
      if (!(pmid in current)) {
        return current;
      }
      const next = { ...current };
      delete next[pmid];
      return next;
    });
  }

  function setDraftValue(pmid: string, value: DraftValue) {
    setDrafts((current) => ({
      ...current,
      [pmid]: value,
    }));
    clearFieldError(pmid);
  }

  async function refreshSettings() {
    await refetch();
  }

  async function saveSetting(property: SettingResponse) {
    let body: UpdateSettingRequest;
    try {
      body = buildRequestBody(property, drafts[property.pmid]);
    } catch (error) {
      const message = getErrorMessage(error);
      setFieldErrors((current) => ({
        ...current,
        [property.pmid]: {
          message,
          path: '/',
        },
      }));
      toast.error(message);
      return;
    }

    setBusy({
      op: 'save',
      pmid: property.pmid,
    });

    try {
      await updateSettingMutation.mutateAsync({
        params: {
          path: {
            pmid: property.pmid,
          },
        },
        body,
      });

      setDrafts((current) => {
        if (!(property.pmid in current)) {
          return current;
        }
        const next = { ...current };
        delete next[property.pmid];
        return next;
      });
      clearFieldError(property.pmid);
      await refetch();
      toast.success(`Saved ${property.label}.`);
    } catch (error) {
      const structured = extractStructuredError(error);
      if (structured) {
        setFieldErrors((current) => ({
          ...current,
          [property.pmid]: structured,
        }));
      }
      toast.error(getErrorMessage(error));
    } finally {
      setBusy(null);
    }
  }

  async function resetSetting(property: SettingResponse) {
    setBusy({
      op: 'reset',
      pmid: property.pmid,
    });

    try {
      await updateSettingMutation.mutateAsync({
        params: {
          path: {
            pmid: property.pmid,
          },
        },
        body: {
          op: 'unset',
        },
      });

      setDrafts((current) => {
        if (!(property.pmid in current)) {
          return current;
        }
        const next = { ...current };
        delete next[property.pmid];
        return next;
      });
      clearFieldError(property.pmid);
      await refetch();
      toast.success(`Reset ${property.label} to default.`);
    } catch (error) {
      const structured = extractStructuredError(error);
      if (structured) {
        setFieldErrors((current) => ({
          ...current,
          [property.pmid]: structured,
        }));
      }
      toast.error(getErrorMessage(error));
    } finally {
      setBusy(null);
    }
  }

  if (isLoading) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center">
        <div className="flex items-center gap-3 rounded-full border border-border/60 bg-card px-5 py-3 text-sm text-muted-foreground shadow-sm">
          <Loader2 className="h-4 w-4 animate-spin" />
          Loading settings document...
        </div>
      </div>
    );
  }

  if (!data) {
    return (
      <div className="mx-auto flex max-w-3xl flex-col gap-4 py-10">
        <Alert variant="destructive">
          <TriangleAlert className="h-4 w-4" />
          <AlertTitle>Settings failed to load</AlertTitle>
          <AlertDescription>
            {getErrorMessage(error ?? new Error('Unknown settings error.'))}
          </AlertDescription>
        </Alert>
        <div>
          <Button onClick={refreshSettings}>
            <RefreshCw className="mr-2 h-4 w-4" />
            Try again
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-6 pb-10">
        <Card className="overflow-hidden border-border/70 bg-[linear-gradient(135deg,color-mix(in_oklab,var(--card)_92%,white),color-mix(in_oklab,var(--muted)_50%,transparent))] shadow-[0_24px_70px_-48px_color-mix(in_oklab,var(--foreground)_32%,transparent)]">
          <CardHeader className="gap-5 border-b border-border/60">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
              <div className="space-y-2">
                <div className="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/70 px-3 py-1 text-xs uppercase tracking-[0.24em] text-muted-foreground">
                  <Settings2 className="h-3.5 w-3.5" />
                  Pure PMID Settings
                </div>
                <CardTitle className="text-3xl tracking-tight">Settings</CardTitle>
                <CardDescription className="max-w-3xl text-sm leading-6">
                  This page renders the server-provided settings document directly.
                  Models, backends, and system status are no longer mixed into this view.
                </CardDescription>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="outline">{visiblePropertyCount} visible</Badge>
                <Badge variant="outline">{totalPropertyCount} total</Badge>
                <Badge variant="outline">schema v{data.schema_version}</Badge>
                <Button
                  variant="outline"
                  onClick={refreshSettings}
                  disabled={isRefetching}
                >
                  {isRefetching ? (
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  ) : (
                    <RefreshCw className="mr-2 h-4 w-4" />
                  )}
                  Refresh
                </Button>
              </div>
            </div>

            <div className="grid gap-4 lg:grid-cols-[1fr_auto]">
              <div className="relative">
                <Search className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder="Search by PMID, label, section, or keyword"
                  className="pl-9"
                />
              </div>
              <div className="rounded-xl border border-border/70 bg-background/80 px-4 py-3 text-sm text-muted-foreground">
                <p className="font-medium text-foreground">settings.json</p>
                <p className="mt-1 break-all">{data.settings_path}</p>
              </div>
            </div>
          </CardHeader>
        </Card>

        {data.warnings.length > 0 ? (
          <Alert>
            <TriangleAlert className="h-4 w-4" />
            <AlertTitle>Recovered settings warnings</AlertTitle>
            <AlertDescription>
              <div className="space-y-1">
                {data.warnings.map((warning) => (
                  <p key={warning}>{warning}</p>
                ))}
              </div>
            </AlertDescription>
          </Alert>
        ) : null}

        {filteredSections.length === 0 ? (
          <Card>
            <CardHeader>
              <CardTitle>No settings matched</CardTitle>
              <CardDescription>
                Clear the search query to see the full settings document.
              </CardDescription>
            </CardHeader>
          </Card>
        ) : null}

        <div className="space-y-6">
          {filteredSections.map((section) => (
            <Card
              key={section.id}
              className="overflow-hidden border-border/70 shadow-[0_20px_60px_-48px_color-mix(in_oklab,var(--foreground)_32%,transparent)]"
            >
              <CardHeader className="gap-3 border-b border-border/60 bg-muted/15">
                <div className="flex flex-wrap items-center gap-3">
                  <CardTitle className="text-2xl">{section.title}</CardTitle>
                  <Badge variant="outline">{section.id}</Badge>
                </div>
                {section.description_md ? (
                  <CardDescription className="max-w-4xl text-sm leading-6">
                    {section.description_md}
                  </CardDescription>
                ) : null}
              </CardHeader>
              <CardContent className="space-y-8 pt-6">
                {section.subsections.map((subsection, subsectionIndex) => (
                  <div key={subsection.id} className="space-y-5">
                    {subsectionIndex > 0 ? <Separator /> : null}
                    <div className="space-y-2">
                      <div className="flex flex-wrap items-center gap-3">
                        <h2 className="text-lg font-semibold">{subsection.title}</h2>
                        <Badge variant="secondary">{subsection.id}</Badge>
                      </div>
                      {subsection.description_md ? (
                        <p className="text-sm leading-6 text-muted-foreground">
                          {subsection.description_md}
                        </p>
                      ) : null}
                    </div>

                    <div className="space-y-4">
                      {subsection.properties.map((property) => {
                        const fieldError = fieldErrors[property.pmid];
                        const isBusy = busy?.pmid === property.pmid;
                        const draftValue = drafts[property.pmid];

                        return (
                          <SettingFieldCard
                            key={property.pmid}
                            property={property}
                            draftValue={draftValue}
                            errorState={fieldError}
                            isBusy={isBusy}
                            busyOp={busy?.op ?? null}
                            onChange={setDraftValue}
                            onSave={saveSetting}
                            onReset={resetSetting}
                          />
                        );
                      })}
                    </div>
                  </div>
                ))}
              </CardContent>
            </Card>
          ))}
        </div>
      </div>
    </div>
  );
}

function SettingFieldCard({
  property,
  draftValue,
  errorState,
  isBusy,
  busyOp,
  onChange,
  onSave,
  onReset,
}: {
  property: SettingResponse;
  draftValue: DraftValue | undefined;
  errorState?: FieldErrorState;
  isBusy: boolean;
  busyOp: NonNullable<BusyState>['op'] | null;
  onChange: (pmid: string, value: DraftValue) => void;
  onSave: (property: SettingResponse) => void;
  onReset: (property: SettingResponse) => void;
}) {
  const propertyType = property.schema.type;
  const isEnum =
    propertyType === 'string' &&
    Array.isArray(property.schema.enum) &&
    property.schema.enum.length > 0;

  const textValue =
    typeof draftValue === 'string'
      ? draftValue
      : valueToEditorString(property.effective_value);
  const booleanValue =
    typeof draftValue === 'boolean'
      ? draftValue
      : Boolean(property.effective_value);

  return (
    <div
      id={`setting-${property.pmid}`}
      className={cn(
        'rounded-2xl border border-border/70 bg-background/70 p-5 transition-colors',
        errorState && 'border-destructive/70 bg-destructive/5',
      )}
    >
      <div className="flex flex-col gap-5 xl:flex-row xl:items-start xl:justify-between">
        <div className="min-w-0 flex-1 space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-base font-semibold">{property.label}</h3>
            <Badge variant={property.is_overridden ? 'default' : 'outline'}>
              {property.is_overridden ? 'Override' : 'Default'}
            </Badge>
            <Badge variant="outline">{propertyType}</Badge>
            {property.schema.secret ? <Badge variant="outline">secret</Badge> : null}
            {property.schema.multiline ? <Badge variant="outline">multiline</Badge> : null}
          </div>

          <code className="block rounded-lg bg-muted/70 px-3 py-2 text-xs text-muted-foreground">
            {property.pmid}
          </code>

          {property.description_md ? (
            <p className="text-sm leading-6 text-muted-foreground">
              {property.description_md}
            </p>
          ) : null}

          <div className="grid gap-3 text-xs text-muted-foreground sm:grid-cols-2">
            <div className="rounded-xl border border-border/60 bg-muted/20 px-3 py-2">
              <p className="font-medium text-foreground">Effective value</p>
              <p className="mt-1 break-all">{summarizeValue(property.effective_value)}</p>
            </div>
            <div className="rounded-xl border border-border/60 bg-muted/20 px-3 py-2">
              <p className="font-medium text-foreground">Default value</p>
              <p className="mt-1 break-all">{summarizeValue(property.schema.default_value)}</p>
            </div>
          </div>
        </div>

        <div className="w-full max-w-xl space-y-4">
          <div className="space-y-2">
            <Label htmlFor={property.pmid}>Value</Label>
            {propertyType === 'boolean' ? (
              <div className="flex items-center justify-between rounded-xl border border-border/70 bg-muted/10 px-4 py-3">
                <div className="text-sm text-muted-foreground">
                  Toggle the effective boolean value for this setting.
                </div>
                <Switch
                  id={property.pmid}
                  checked={booleanValue}
                  onCheckedChange={(value) => onChange(property.pmid, value)}
                  disabled={isBusy}
                />
              </div>
            ) : isEnum ? (
              <Select
                value={textValue}
                onValueChange={(value) => onChange(property.pmid, value)}
                disabled={isBusy}
              >
                <SelectTrigger id={property.pmid} className="w-full">
                  <SelectValue placeholder="Select a value" />
                </SelectTrigger>
                <SelectContent>
                  {property.schema.enum?.map((option) => (
                    <SelectItem key={option} value={option}>
                      {option}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            ) : propertyType === 'integer' ? (
              <Input
                id={property.pmid}
                inputMode="numeric"
                value={textValue}
                onChange={(event) => onChange(property.pmid, event.target.value)}
                placeholder="Leave blank to reset to default"
                aria-invalid={Boolean(errorState)}
                disabled={isBusy}
              />
            ) : propertyType === 'array' || propertyType === 'object' || property.schema.multiline ? (
              <Textarea
                id={property.pmid}
                value={textValue}
                onChange={(event) => onChange(property.pmid, event.target.value)}
                placeholder={
                  propertyType === 'array' || propertyType === 'object'
                    ? 'Enter valid JSON'
                    : undefined
                }
                className="min-h-48 font-mono text-xs"
                aria-invalid={Boolean(errorState)}
                disabled={isBusy}
              />
            ) : (
              <Input
                id={property.pmid}
                type={property.schema.secret ? 'password' : 'text'}
                value={textValue}
                onChange={(event) => onChange(property.pmid, event.target.value)}
                aria-invalid={Boolean(errorState)}
                disabled={isBusy}
              />
            )}
            {errorState ? (
              <p className="text-sm text-destructive">
                {errorState.message} <span className="text-xs text-destructive/80">({errorState.path})</span>
              </p>
            ) : null}
          </div>

          <div className="flex flex-wrap justify-end gap-2">
            <Button
              variant="outline"
              onClick={() => onReset(property)}
              disabled={isBusy}
            >
              {isBusy && busyOp === 'reset' ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <RotateCcw className="mr-2 h-4 w-4" />
              )}
              Reset
            </Button>
            <Button onClick={() => onSave(property)} disabled={isBusy}>
              {isBusy && busyOp === 'save' ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <Save className="mr-2 h-4 w-4" />
              )}
              Save
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
