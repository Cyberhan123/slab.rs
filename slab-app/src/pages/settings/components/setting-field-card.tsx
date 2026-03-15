import {
  CheckCircle2,
  Clock3,
  Loader2,
  RotateCcw,
  TriangleAlert,
} from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { cn } from '@/lib/utils';

import type {
  DraftValue,
  FieldErrorState,
  FieldStatusState,
  SettingResponse,
} from '../types';
import { summarizeValue, valueToEditorString } from '../utils';

type SettingFieldCardProps = {
  property: SettingResponse;
  draftValue: DraftValue | undefined;
  errorState?: FieldErrorState;
  fieldStatus?: FieldStatusState;
  isResetting: boolean;
  onChange: (property: SettingResponse, value: DraftValue) => void;
  onReset: (property: SettingResponse) => void;
};

export function SettingFieldCard({
  property,
  draftValue,
  errorState,
  fieldStatus,
  isResetting,
  onChange,
  onReset,
}: SettingFieldCardProps) {
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
            <div className="flex items-center justify-between gap-3">
              <Label htmlFor={property.pmid}>Value</Label>
              <FieldStatusBadge status={fieldStatus} />
            </div>

            {propertyType === 'boolean' ? (
              <div className="flex items-center justify-between rounded-xl border border-border/70 bg-muted/10 px-4 py-3">
                <div className="text-sm text-muted-foreground">
                  Toggle the effective boolean value for this setting.
                </div>
                <Switch
                  id={property.pmid}
                  checked={booleanValue}
                  onCheckedChange={(value) => onChange(property, value)}
                />
              </div>
            ) : isEnum ? (
              <Select value={textValue} onValueChange={(value) => onChange(property, value)}>
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
                onChange={(event) => onChange(property, event.target.value)}
                placeholder="Leave blank to reset to default"
                aria-invalid={Boolean(errorState)}
              />
            ) : propertyType === 'array' ||
              propertyType === 'object' ||
              property.schema.multiline ? (
              <Textarea
                id={property.pmid}
                value={textValue}
                onChange={(event) => onChange(property, event.target.value)}
                placeholder={
                  propertyType === 'array' || propertyType === 'object'
                    ? 'Enter valid JSON'
                    : undefined
                }
                className="min-h-48 font-mono text-xs"
                aria-invalid={Boolean(errorState)}
              />
            ) : (
              <Input
                id={property.pmid}
                type={property.schema.secret ? 'password' : 'text'}
                value={textValue}
                onChange={(event) => onChange(property, event.target.value)}
                aria-invalid={Boolean(errorState)}
              />
            )}

            {errorState ? (
              <p className="text-sm text-destructive">
                {errorState.message}{' '}
                <span className="text-xs text-destructive/80">({errorState.path})</span>
              </p>
            ) : null}
          </div>

          <div className="flex flex-wrap justify-end gap-2">
            <Button
              variant="outline"
              onClick={() => onReset(property)}
              disabled={isResetting}
            >
              {isResetting ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <RotateCcw className="mr-2 h-4 w-4" />
              )}
              Reset
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function FieldStatusBadge({ status }: { status?: FieldStatusState }) {
  if (!status) {
    return <span className="text-xs text-muted-foreground">Auto-save ready</span>;
  }

  if (status.tone === 'saving') {
    return (
      <span className="inline-flex items-center gap-2 text-xs text-muted-foreground">
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
        {status.message}
      </span>
    );
  }

  if (status.tone === 'saved') {
    return (
      <span className="inline-flex items-center gap-2 text-xs text-emerald-600">
        <CheckCircle2 className="h-3.5 w-3.5" />
        {status.message}
      </span>
    );
  }

  if (status.tone === 'error') {
    return (
      <span className="inline-flex items-center gap-2 text-xs text-destructive">
        <TriangleAlert className="h-3.5 w-3.5" />
        {status.message}
      </span>
    );
  }

  return (
    <span className="inline-flex items-center gap-2 text-xs text-amber-700">
      <Clock3 className="h-3.5 w-3.5" />
      {status.message}
    </span>
  );
}
