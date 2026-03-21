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
import { SoftPanel, StatusPill } from '@/components/ui/workspace';
import { cn } from '@/lib/utils';

import { parseStructuredJsonSchema } from '../schema';
import type {
  DraftValue,
  FieldErrorState,
  FieldStatusState,
  SettingResponse,
} from '../types';
import { valueToEditorString } from '../utils';
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
    <SoftPanel
      id={`setting-${property.pmid}`}
      className={cn(
        'space-y-4 rounded-[26px] border border-border/70 p-4',
        errorState && 'border-destructive/70 bg-destructive/5',
      )}
    >
      <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0 flex-1 space-y-2">
          <h3 className="text-base font-semibold tracking-tight">{property.label}</h3>

          {property.description_md ? (
            <p className="text-sm leading-6 text-muted-foreground">
              {property.description_md}
            </p>
          ) : null}
        </div>

        <div className="flex items-center justify-between gap-3 lg:min-w-[12rem] lg:flex-col lg:items-end">
          <FieldStatusBadge status={fieldStatus} />
          <Button
            variant="pill"
            size="sm"
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
          <div className="workspace-soft-panel flex items-center justify-between rounded-2xl px-4 py-3">
            <div className="text-sm text-muted-foreground">
              {booleanValue ? 'Enabled' : 'Disabled'}
            </div>
            <Switch
              id={property.pmid}
              variant="workspace"
              checked={booleanValue}
              onCheckedChange={(value) => onChange(property, value)}
            />
          </div>
        ) : isEnum ? (
          <Select value={textValue} onValueChange={(value) => onChange(property, value)}>
            <SelectTrigger id={property.pmid} variant="soft" className="h-11 w-full">
              <SelectValue placeholder="Select an option" />
            </SelectTrigger>
            <SelectContent variant="soft">
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
            variant="soft"
            value={textValue}
            onChange={(event) => onChange(property, event.target.value)}
            placeholder="Enter a whole number"
            className="h-11"
            aria-invalid={Boolean(errorState)}
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
            variant="soft"
            value={textValue}
            onChange={(event) => onChange(property, event.target.value)}
            placeholder={
              propertyType === 'array' || propertyType === 'object'
                ? 'Enter valid JSON'
                : 'Enter a value'
            }
            className="min-h-48 font-mono text-xs"
            aria-invalid={Boolean(errorState)}
          />
        ) : (
          <Input
            id={property.pmid}
            type={property.schema.secret ? 'password' : 'text'}
            variant="soft"
            value={textValue}
            onChange={(event) => onChange(property, event.target.value)}
            placeholder="Enter a value"
            className="h-11"
            aria-invalid={Boolean(errorState)}
          />
        )}

        {errorState && !structuredSchema ? (
          <p className="text-sm text-destructive">{errorState.message}</p>
        ) : null}
      </div>
    </SoftPanel>
  );
}

function FieldStatusBadge({ status }: { status?: FieldStatusState }) {
  if (!status) {
    return <StatusPill status="neutral">Auto-save ready</StatusPill>;
  }

  if (status.tone === 'saving') {
    return (
      <StatusPill status="info" className="gap-2">
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
        {status.message}
      </StatusPill>
    );
  }

  if (status.tone === 'saved') {
    return (
      <StatusPill status="success" className="gap-2">
        <CheckCircle2 className="h-3.5 w-3.5" />
        {status.message}
      </StatusPill>
    );
  }

  if (status.tone === 'error') {
    return (
      <StatusPill status="danger" className="gap-2">
        <TriangleAlert className="h-3.5 w-3.5" />
        {status.message}
      </StatusPill>
    );
  }

  return (
    <StatusPill status="neutral" className="gap-2">
      <Clock3 className="h-3.5 w-3.5" />
      {status.message}
    </StatusPill>
  );
}
