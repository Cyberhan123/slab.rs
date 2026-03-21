import { useDeferredValue, useMemo, useState } from 'react';
import {
  Loader2,
  RefreshCw,
  Search,
  SlidersHorizontal,
  TriangleAlert,
} from 'lucide-react';

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
import {
  MetricCard,
  PillFilterBar,
  SoftPanel,
  StageEmptyState,
} from '@/components/ui/workspace';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import api, { getErrorMessage } from '@/lib/api';

import { SettingFieldCard } from './components/setting-field-card';
import { SettingsNavigation } from './components/settings-navigation';
import { useSettingsAutosave } from './hooks/use-settings-autosave';
import type { SettingResponse } from './types';
import {
  countProperties,
  countSectionProperties,
  matchesSearch,
  sectionAnchorId,
  subsectionAnchorId,
  shouldCollapseSubsectionHeading,
} from './utils';

export default function SettingsPage() {
  const [search, setSearch] = useState('');
  const [activeTarget, setActiveTarget] = useState<string | null>(null);

  const deferredSearch = useDeferredValue(search);
  const normalizedSearch = deferredSearch.trim().toLowerCase();

  const { data, error, isLoading, isRefetching, refetch } = api.useQuery('get', '/v1/settings');

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

  const totalPropertyCount = useMemo(() => countProperties(data?.sections ?? []), [data]);
  const visiblePropertyCount = useMemo(() => countProperties(filteredSections), [filteredSections]);
  const sectionCount = data?.sections.length ?? 0;

  usePageHeader({
    ...PAGE_HEADER_META.settings,
    subtitle:
      normalizedSearch.length > 0
        ? `Showing ${visiblePropertyCount} of ${totalPropertyCount} settings for "${deferredSearch.trim()}"`
        : PAGE_HEADER_META.settings.subtitle,
  });

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

  function jumpToTarget(targetId: string) {
    setActiveTarget(targetId);
    document.getElementById(targetId)?.scrollIntoView({
      behavior: 'smooth',
      block: 'start',
    });
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
    <div className="h-full w-full overflow-y-auto">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-5 px-1 pb-8">
        <SoftPanel className="workspace-halo space-y-5 overflow-hidden rounded-[30px] border border-border/70">
          <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
            <div className="space-y-2">
              <Badge variant="chip">Dynamic schema settings</Badge>
              <h1 className="text-2xl font-semibold tracking-tight md:text-3xl">Settings Workbench</h1>
              <p className="max-w-3xl text-sm leading-6 text-muted-foreground">
                Configure runtime and product options with auto-save and schema-aware validation.
              </p>
            </div>
            <Button
              variant="pill"
              size="pill"
              disabled={isRefetching}
              onClick={() => void refetch()}
            >
              {isRefetching ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <RefreshCw className="mr-2 h-4 w-4" />
              )}
              Refresh
            </Button>
          </div>

          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <MetricCard label="Sections" value={sectionCount} hint="Top-level configuration groups" icon={SlidersHorizontal} />
            <MetricCard label="Visible Fields" value={visiblePropertyCount} hint="After current search filter" icon={Search} />
            <MetricCard label="Pending Saves" value={statusSummary.dirty} hint="Waiting for autosave" icon={Loader2} />
            <MetricCard label="Errors" value={statusSummary.error} hint="Validation or save errors" icon={TriangleAlert} />
          </div>
        </SoftPanel>

        <PillFilterBar>
          <div className="relative min-w-[280px] flex-1">
            <Search className="pointer-events-none absolute top-1/2 left-4 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              variant="shell"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search settings"
              className="h-10 w-full pl-10"
            />
          </div>
          <Badge variant="counter">
            {visiblePropertyCount} / {totalPropertyCount} fields
          </Badge>
          {statusSummary.saving > 0 ? <Badge variant="chip">{statusSummary.saving} saving</Badge> : null}
        </PillFilterBar>

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
          <StageEmptyState
            icon={Search}
            title="No settings matched"
            description="Try a broader keyword or clear search."
          />
        ) : (
          <div className="grid gap-6 lg:grid-cols-[300px_minmax(0,1fr)]">
            <aside className="lg:sticky lg:top-3 lg:self-start">
              <SettingsNavigation
                activeTarget={activeTarget}
                dirtyCount={statusSummary.dirty}
                errorCount={statusSummary.error}
                savingCount={statusSummary.saving}
                sections={filteredSections}
                onJump={jumpToTarget}
              />
            </aside>

            <div className="space-y-5">
              {filteredSections.map((section) => (
                <Card
                  key={section.id}
                  id={sectionAnchorId(section.id)}
                  variant="soft"
                  className="scroll-mt-20 overflow-hidden rounded-[30px]"
                >
                  <CardHeader className="gap-3 border-b border-border/60 bg-[var(--surface-soft)]/70">
                    <div className="flex flex-wrap items-center gap-3">
                      <CardTitle className="text-2xl tracking-tight">{section.title}</CardTitle>
                      <Badge variant="counter">
                        {countSectionProperties(section)} settings
                      </Badge>
                    </div>
                    {section.description_md ? (
                      <CardDescription className="max-w-4xl text-sm leading-6">
                        {section.description_md}
                      </CardDescription>
                    ) : null}
                  </CardHeader>

                  <CardContent className="space-y-7 pt-5">
                    {section.subsections.map((subsection, subsectionIndex) => (
                      <div
                        key={subsection.id}
                        id={subsectionAnchorId(section.id, subsection.id)}
                        className="scroll-mt-24 space-y-4"
                      >
                        {subsectionIndex > 0 ? <SeparatorLine /> : null}
                        {shouldCollapseSubsectionHeading(section, subsection) ? (
                          subsection.description_md ? (
                            <p className="text-sm leading-6 text-muted-foreground">
                              {subsection.description_md}
                            </p>
                          ) : null
                        ) : (
                          <div className="space-y-2">
                            <div className="flex flex-wrap items-center gap-3">
                              <h2 className="text-lg font-semibold tracking-tight">{subsection.title}</h2>
                              <Badge variant="chip">{subsection.properties.length} fields</Badge>
                            </div>
                            {subsection.description_md ? (
                              <p className="text-sm leading-6 text-muted-foreground">
                                {subsection.description_md}
                              </p>
                            ) : null}
                          </div>
                        )}

                        <div className="space-y-3">
                          {subsection.properties.map((property) => (
                            <SettingFieldCard
                              key={property.pmid}
                              property={property}
                              draftValue={drafts[property.pmid]}
                              errorState={fieldErrors[property.pmid]}
                              fieldStatus={fieldStatuses[property.pmid]}
                              isResetting={resettingPmid === property.pmid}
                              onChange={setDraftValue}
                              onReset={resetSetting}
                            />
                          ))}
                        </div>
                      </div>
                    ))}
                  </CardContent>
                </Card>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function SeparatorLine() {
  return <div className="h-px w-full bg-border/60" />;
}
