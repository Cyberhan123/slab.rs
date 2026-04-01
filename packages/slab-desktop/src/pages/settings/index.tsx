import { useEffect, useMemo, useRef, useState } from 'react';
import { Loader2, RefreshCw, TriangleAlert } from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import { StageEmptyState, StatusPill } from '@slab/components/workspace';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import api, { getErrorMessage } from '@/lib/api';

import { SettingFieldCard } from './components/setting-field-card';
import { SettingsNavigation } from './components/settings-navigation';
import { useSettingsAutosave } from './hooks/use-settings-autosave';
import type { SettingResponse } from './types';
import {
  countSectionProperties,
  sectionAnchorId,
  subsectionAnchorId,
  shouldCollapseSubsectionHeading,
} from './utils';

export default function SettingsPage() {
  const canvasRef = useRef<HTMLDivElement | null>(null);
  const [activeSectionId, setActiveSectionId] = useState<string | null>(null);

  const { data, error, isLoading, refetch } = api.useQuery('get', '/v1/settings');

  const propertyMap = useMemo(() => {
    const map = new Map<string, SettingResponse>();

    for (const section of data?.sections ?? []) {
      for (const subsection of section.subsections) {
        for (const property of subsection.properties) {
          map.set(property.pmid, property);
        }
      }
    }

    return map;
  }, [data]);

  const sections = data?.sections ?? [];
  const activeSection = useMemo(
    () => sections.find((section) => section.id === activeSectionId) ?? sections[0] ?? null,
    [activeSectionId, sections],
  );

  useEffect(() => {
    if (sections.length === 0) {
      setActiveSectionId(null);
      return;
    }

    if (activeSectionId && sections.some((section) => section.id === activeSectionId)) {
      return;
    }

    const nextSectionId = sections[0].id;
    setActiveSectionId(nextSectionId);
  }, [activeSectionId, sections]);

  usePageHeader(PAGE_HEADER_META.settings);

  const {
    drafts,
    fieldErrors,
    fieldStatuses,
    resettingPmid,
    statusSummary,
    setDraftValue,
    resetSetting,
  } = useSettingsAutosave({
    propertyMap,
    refetch,
  });

  function scrollToTarget(targetId: string) {
    window.requestAnimationFrame(() => {
      document.getElementById(targetId)?.scrollIntoView({
        behavior: 'smooth',
        block: 'start',
      });
    });
  }

  function selectSection(sectionId: string) {
    const targetId = sectionAnchorId(sectionId);
    setActiveSectionId(sectionId);
    canvasRef.current?.scrollTo({ top: 0, behavior: 'smooth' });
    scrollToTarget(targetId);
  }

  if (isLoading) {
    return (
      <StageEmptyState
        icon={Loader2}
        title="Loading settings document"
        description="Fetching runtime schema and values."
        className="[&_svg]:animate-spin"
      />
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
          <Button variant="pill" size="pill" onClick={() => void refetch()}>
            <RefreshCw className="mr-2 h-4 w-4" />
            Try again
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col overflow-hidden rounded-[24px] border border-border/50 bg-[var(--shell-card)] shadow-[var(--shell-elevation)] lg:flex-row">
      <aside className="w-full shrink-0 border-b border-border/50 bg-[var(--surface-soft)]/80 lg:w-[256px] lg:border-r lg:border-b-0">
        <SettingsNavigation
          activeSectionId={activeSection?.id ?? null}
          sections={sections}
          onSelectSection={selectSection}
        />
      </aside>

      <div ref={canvasRef} className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto flex w-full max-w-[944px] flex-col gap-6 px-6 py-6 md:px-8 md:py-8">
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

          {!activeSection ? (
            <StageEmptyState
              title="No settings available"
              description="The settings document is empty."
            />
          ) : (
            <>
              <header
                id={sectionAnchorId(activeSection.id)}
                className="scroll-mt-6 space-y-4 border-b border-border/50 pb-6"
              >
                <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
                  <div className="space-y-3">
                    <div className="flex flex-wrap items-center gap-3">
                      <h1 className="text-3xl font-bold tracking-[-0.045em] text-foreground">
                        {activeSection.title}
                      </h1>
                      <Badge
                        variant="chip"
                        className="rounded-full border-border/60 bg-border/30 px-3 py-1 text-[10px] font-bold uppercase tracking-[0.08em] text-muted-foreground"
                      >
                        {countSectionProperties(activeSection)} settings
                      </Badge>
                    </div>
                    {activeSection.description_md ? (
                      <p className="max-w-3xl text-base leading-8 text-muted-foreground">
                        {activeSection.description_md}
                      </p>
                    ) : null}
                  </div>

                  {statusSummary.error > 0 ||
                  statusSummary.saving > 0 ||
                  statusSummary.dirty > 0 ? (
                    <div className="flex flex-wrap items-center gap-2">
                      {statusSummary.error > 0 ? (
                        <StatusPill status="danger">
                          {statusSummary.error} issue{statusSummary.error > 1 ? 's' : ''}
                        </StatusPill>
                      ) : null}
                      {statusSummary.saving > 0 ? (
                        <StatusPill status="info">{statusSummary.saving} saving</StatusPill>
                      ) : null}
                      {statusSummary.dirty > 0 ? (
                        <Badge variant="counter">{statusSummary.dirty} pending</Badge>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              </header>

              <div className="space-y-6 pb-8">
                {activeSection.subsections.map((subsection) => (
                  <section
                    key={subsection.id}
                    id={subsectionAnchorId(activeSection.id, subsection.id)}
                    className="scroll-mt-8 rounded-[20px] border border-border/40 bg-[var(--surface-soft)]/70 p-6 md:p-8"
                  >
                    {shouldCollapseSubsectionHeading(activeSection, subsection) ? (
                      subsection.description_md ? (
                        <p className="text-sm leading-7 text-muted-foreground">
                          {subsection.description_md}
                        </p>
                      ) : null
                    ) : (
                      <div className="space-y-2">
                        <div className="flex flex-wrap items-center gap-3">
                          <h2 className="text-[18px] font-bold tracking-[-0.03em] text-foreground">
                            {subsection.title}
                          </h2>
                        </div>
                        {subsection.description_md ? (
                          <p className="text-sm leading-7 text-muted-foreground">
                            {subsection.description_md}
                          </p>
                        ) : null}
                      </div>
                    )}

                    <div className="mt-6 grid gap-4 xl:grid-cols-2">
                      {subsection.properties.map((property) => (
                        <div
                          key={property.pmid}
                          className={shouldPropertySpanFullWidth(property) ? 'xl:col-span-2' : ''}
                        >
                          <SettingFieldCard
                            property={property}
                            draftValue={drafts[property.pmid]}
                            errorState={fieldErrors[property.pmid]}
                            fieldStatus={fieldStatuses[property.pmid]}
                            isResetting={resettingPmid === property.pmid}
                            onChange={setDraftValue}
                            onReset={resetSetting}
                          />
                        </div>
                      ))}
                    </div>
                  </section>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function shouldPropertySpanFullWidth(property: SettingResponse) {
  if (property.schema.multiline || property.schema.json_schema) {
    return true;
  }

  if (property.schema.type === 'array' || property.schema.type === 'object') {
    return true;
  }

  const label = property.label.toLowerCase();

  return (
    property.schema.type === 'boolean' ||
    label.includes('directory') ||
    label.includes('providers') ||
    label.includes('path')
  );
}
