import {
  CheckCircle2,
  Clock3,
  Loader2,
  RotateCcw,
  TriangleAlert,
} from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
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

import { parseStructuredJsonSchema } from '../schema';
import type {
  DraftValue,
  FieldErrorState,
  FieldStatusState,
  SettingResponse,
} from '../types';
import { valueToEditorString } from '../utils';
import {
  ChatProvidersField,
  supportsChatProvidersField,
} from './chat-providers-field';
import { StructuredJsonField } from './structured-json-field';

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
  const structuredSchema = parseStructuredJsonSchema(property);
  const useChatProvidersField = supportsChatProvidersField(property);
  const isEnum =
    propertyType === 'string' &&
    Array.isArray(property.schema.enum) &&
    property.schema.enum.length > 0;

  const textValue =
    typeof draftValue === 'string'
      ? draftValue
      : draftValue !== undefined
        ? valueToEditorString(draftValue)
        : valueToEditorString(property.effective_value);
  const booleanValue =
    typeof draftValue === 'boolean'
      ? draftValue
      : Boolean(property.effective_value);
  const structuredValue: DraftValue =
    draftValue !== undefined &&
    typeof draftValue !== 'boolean' &&
    typeof draftValue !== 'string'
      ? draftValue
      : (property.effective_value as DraftValue);
  const canReset = property.is_overridden || draftValue !== undefined;

  return (
    <div
      id={`setting-${property.pmid}`}
      className={cn(
        'rounded-3xl border border-border/70 bg-background/80 p-5 shadow-[0_18px_50px_-44px_color-mix(in_oklab,var(--foreground)_34%,transparent)] transition-colors',
        errorState && 'border-destructive/70 bg-destructive/5',
      )}
    >
      <div className="space-y-4">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div className="min-w-0 flex-1 space-y-2">
            <h3 className="text-base font-semibold">{property.label}</h3>

            {property.description_md ? (
              <p className="text-sm leading-6 text-muted-foreground">
                {property.description_md}
              </p>
            ) : null}
          </div>

          <div className="flex items-center justify-between gap-3 lg:min-w-[11rem] lg:flex-col lg:items-end">
            <FieldStatusBadge status={fieldStatus} />
            <Button
              variant="outline"
              onClick={() => onReset(property)}
              disabled={isResetting || !canReset}
              className="min-w-28"
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

        <div className="space-y-2">
          {propertyType === 'boolean' ? (
            <div className="flex items-center justify-between rounded-2xl border border-border/70 bg-muted/10 px-4 py-3">
              <div className="text-sm text-muted-foreground">
                {booleanValue ? 'Enabled' : 'Disabled'}
              </div>
              <Switch
                id={property.pmid}
                checked={booleanValue}
                onCheckedChange={(value) => onChange(property, value)}
              />
            </div>
          ) : isEnum ? (
            <Select value={textValue} onValueChange={(value) => onChange(property, value)}>
              <SelectTrigger id={property.pmid} className="h-11 w-full rounded-2xl">
                <SelectValue placeholder="Select an option" />
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
              placeholder="Enter a whole number"
              className="h-11 rounded-2xl"
              aria-invalid={Boolean(errorState)}
            />
          ) : useChatProvidersField ? (
            <ChatProvidersField
              value={structuredValue}
              errorState={errorState}
              onChange={(value) => onChange(property, value)}
            />
          ) : structuredSchema ? (
            <StructuredJsonField
              schema={structuredSchema}
              value={structuredValue}
              errorState={errorState}
              onChange={(value) => onChange(property, value)}
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
                  : 'Enter a value'
              }
              className="min-h-48 rounded-3xl font-mono text-xs"
              aria-invalid={Boolean(errorState)}
            />
          ) : (
            <Input
              id={property.pmid}
              type={property.schema.secret ? 'password' : 'text'}
              value={textValue}
              onChange={(event) => onChange(property, event.target.value)}
              placeholder="Enter a value"
              className="h-11 rounded-2xl"
              aria-invalid={Boolean(errorState)}
            />
          )}

          {errorState && !structuredSchema && !useChatProvidersField ? (
            <p className="text-sm text-destructive">{errorState.message}</p>
          ) : null}
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
