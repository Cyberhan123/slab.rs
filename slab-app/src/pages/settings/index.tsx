import { useDeferredValue, useMemo, useState } from 'react';
import { Clock3, Loader2, RefreshCw, Search, Settings2, TriangleAlert } from 'lucide-react';

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
} from './utils';

export default function SettingsPage() {
  const [search, setSearch] = useState('');
  const [activeTarget, setActiveTarget] = useState<string | null>(null);

  const deferredSearch = useDeferredValue(search);
  const normalizedSearch = deferredSearch.trim().toLowerCase();

  const {
    data,
    error,
    isLoading,
    isRefetching,
    refetch,
  } = api.useQuery('get', '/v1/settings');

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

  const totalPropertyCount = useMemo(
    () => countProperties(data?.sections ?? []),
    [data],
  );
  const visiblePropertyCount = useMemo(
    () => countProperties(filteredSections),
    [filteredSections],
  );

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

  async function refreshSettings() {
    await refetch();
  }

  function jumpToTarget(targetId: string) {
    setActiveTarget(targetId);
    document.getElementById(targetId)?.scrollIntoView({
      behavior: 'smooth',
      block: 'start',
    });
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
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-6 pb-10">
        <Card className="overflow-hidden border-border/70 bg-[linear-gradient(135deg,color-mix(in_oklab,var(--card)_92%,white),color-mix(in_oklab,var(--muted)_50%,transparent))] shadow-[0_24px_70px_-48px_color-mix(in_oklab,var(--foreground)_32%,transparent)]">
          <CardHeader className="gap-5 border-b border-border/60">
            <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
              <div className="space-y-2">
                <CardTitle className="text-3xl tracking-tight">Settings</CardTitle>
                <CardDescription className="max-w-3xl text-sm leading-6">
                  Changes auto-save after a short pause. Use the left navigation to jump
                  between sections and sub-sections.
                </CardDescription>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="outline">{visiblePropertyCount} visible</Badge>
                <Badge variant="outline">{totalPropertyCount} total</Badge>
                <Badge variant="outline">schema v{data.schema_version}</Badge>
                {statusSummary.dirty > 0 ? (
                  <Badge variant="secondary">{statusSummary.dirty} pending</Badge>
                ) : null}
                {statusSummary.saving > 0 ? (
                  <Badge variant="secondary">{statusSummary.saving} saving</Badge>
                ) : null}
                {statusSummary.error > 0 ? (
                  <Badge variant="destructive">{statusSummary.error} errors</Badge>
                ) : null}
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

            <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_auto]">
              <div className="relative">
                <Search className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder="Search by PMID, label, section, or keyword"
                  className="pl-9"
                />
              </div>
              <div className="flex items-center gap-2 rounded-xl border border-border/70 bg-background/70 px-4 py-3 text-sm text-muted-foreground">
                <Clock3 className="h-4 w-4" />
                Auto-save is enabled
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

        {filteredSections.length > 0 ? (
          <div className="grid gap-6 lg:grid-cols-[280px_minmax(0,1fr)]">
            <aside className="lg:sticky lg:top-4 lg:self-start">
              <SettingsNavigation
                activeTarget={activeTarget}
                dirtyCount={statusSummary.dirty}
                errorCount={statusSummary.error}
                savingCount={statusSummary.saving}
                sections={filteredSections}
                onJump={jumpToTarget}
              />
            </aside>

            <div className="space-y-6">
              {filteredSections.map((section) => (
                <Card
                  key={section.id}
                  id={sectionAnchorId(section.id)}
                  className="scroll-mt-6 overflow-hidden border-border/70 shadow-[0_20px_60px_-48px_color-mix(in_oklab,var(--foreground)_32%,transparent)]"
                >
                  <CardHeader className="gap-3 border-b border-border/60 bg-muted/15">
                    <div className="flex flex-wrap items-center gap-3">
                      <CardTitle className="text-2xl">{section.title}</CardTitle>
                      <Badge variant="outline">{section.id}</Badge>
                      <Badge variant="secondary">
                        {countSectionProperties(section)} settings
                      </Badge>
                    </div>
                    {section.description_md ? (
                      <CardDescription className="max-w-4xl text-sm leading-6">
                        {section.description_md}
                      </CardDescription>
                    ) : null}
                  </CardHeader>
                  <CardContent className="space-y-8 pt-6">
                    {section.subsections.map((subsection, subsectionIndex) => (
                      <div
                        key={subsection.id}
                        id={subsectionAnchorId(section.id, subsection.id)}
                        className="scroll-mt-28 space-y-5"
                      >
                        {subsectionIndex > 0 ? <SeparatorLine /> : null}
                        <div className="space-y-2">
                          <div className="flex flex-wrap items-center gap-3">
                            <h2 className="text-lg font-semibold">{subsection.title}</h2>
                            <Badge variant="secondary">{subsection.id}</Badge>
                            <Badge variant="outline">
                              {subsection.properties.length} fields
                            </Badge>
                          </div>
                          {subsection.description_md ? (
                            <p className="text-sm leading-6 text-muted-foreground">
                              {subsection.description_md}
                            </p>
                          ) : null}
                        </div>

                        <div className="space-y-4">
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
        ) : null}
      </div>
    </div>
  );
}

function SeparatorLine() {
  return <div className="h-px w-full bg-border/60" />;
}
