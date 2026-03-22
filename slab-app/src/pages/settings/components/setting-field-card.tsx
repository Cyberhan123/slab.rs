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
import { StatusPill } from '@/components/ui/workspace';
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
    <div
      id={`setting-${property.pmid}`}
      className={cn(
        'rounded-[16px] border border-slate-200/80 bg-white p-5 shadow-[0_1px_2px_rgba(15,23,42,0.06)]',
        errorState && 'border-destructive/70 bg-destructive/5',
      )}
    >
      <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
        <div className="min-w-0 flex-1 space-y-1.5">
          <h3 className="text-sm font-bold tracking-[-0.02em] text-slate-900">{property.label}</h3>

          {property.description_md ? (
            <p className="max-w-2xl text-[11px] leading-[16.5px] text-slate-500">
              {property.description_md}
            </p>
          ) : null}
        </div>

        <div className="flex flex-wrap items-center gap-2 self-start">
          <FieldStatusBadge status={fieldStatus} />
          <Button
            variant="outline"
            size="sm"
            onClick={() => onReset(property)}
            disabled={isResetting || !canReset}
            className="h-8 rounded-[12px] border-slate-200 px-3 text-[11px] font-semibold uppercase tracking-[0.08em] text-slate-500 shadow-none"
          >
            {isResetting ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <RotateCcw className="mr-2 h-4 w-4" />
            )}
            Reset
          </Button>

          {propertyType === 'boolean' ? (
            <Switch
              id={property.pmid}
              variant="workspace"
              checked={booleanValue}
              onCheckedChange={(value) => onChange(property, value)}
              className="data-[size=default]:h-[1.35rem] data-[size=default]:w-10"
            />
          ) : null}
        </div>
      </div>

      {propertyType === 'boolean' ? null : (
        <div className="mt-4 space-y-2">
          {isEnum ? (
          <Select value={textValue} onValueChange={(value) => onChange(property, value)}>
            <SelectTrigger
              id={property.pmid}
              variant="soft"
              className="h-[42px] w-full rounded-[12px] border-slate-200 bg-slate-50 px-4 text-xs shadow-[inset_0_0_0_1px_rgba(203,213,225,0.75)]"
            >
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
            className="h-[42px] rounded-[12px] border-slate-200 bg-slate-50 px-4 font-mono text-xs shadow-[inset_0_0_0_1px_rgba(203,213,225,0.75)]"
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
            className="min-h-40 rounded-[12px] border-slate-200 bg-slate-50 px-4 py-3 font-mono text-xs shadow-[inset_0_0_0_1px_rgba(203,213,225,0.75)]"
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
            className="h-[42px] rounded-[12px] border-slate-200 bg-slate-50 px-4 font-mono text-xs shadow-[inset_0_0_0_1px_rgba(203,213,225,0.75)]"
            aria-invalid={Boolean(errorState)}
          />
          )}

          {errorState && !structuredSchema ? (
            <p className="text-sm text-destructive">{errorState.message}</p>
          ) : null}
        </div>
      )}
    </div>
  );
}

function FieldStatusBadge({ status }: { status?: FieldStatusState }) {
  if (!status) {
    return null;
  }

  if (status.tone === 'saving') {
    return (
      <StatusPill status="info" className="h-8 gap-2 rounded-full px-3 text-[11px] font-semibold">
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
        {status.message}
      </StatusPill>
    );
  }

  if (status.tone === 'saved') {
    return (
      <StatusPill status="success" className="h-8 gap-2 rounded-full px-3 text-[11px] font-semibold">
        <CheckCircle2 className="h-3.5 w-3.5" />
        {status.message}
      </StatusPill>
    );
  }

  if (status.tone === 'error') {
    return (
      <StatusPill status="danger" className="h-8 gap-2 rounded-full px-3 text-[11px] font-semibold">
        <TriangleAlert className="h-3.5 w-3.5" />
        {status.message}
      </StatusPill>
    );
  }

  return (
    <StatusPill status="neutral" className="h-8 gap-2 rounded-full px-3 text-[11px] font-semibold">
      <Clock3 className="h-3.5 w-3.5" />
      {status.message}
    </StatusPill>
  );
}
