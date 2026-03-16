import { useDeferredValue, useMemo, useState } from 'react';
import { Loader2, RefreshCw, Search, TriangleAlert } from 'lucide-react';

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
        <div className="sticky top-0 z-30 pb-2">
          <Card className="overflow-hidden border-border/70 bg-background/90 shadow-[0_24px_70px_-48px_color-mix(in_oklab,var(--foreground)_32%,transparent)] backdrop-blur supports-[backdrop-filter]:bg-background/72">
            <CardContent className="py-4">
              <div className="relative">
                <Search className="pointer-events-none absolute top-1/2 left-4 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder="Search settings"
                  className="h-12 rounded-2xl border-border/70 bg-background pl-11 text-base shadow-[inset_0_1px_0_color-mix(in_oklab,var(--foreground)_8%,transparent)]"
                />
              </div>
            </CardContent>
          </Card>
        </div>

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
                Try a broader keyword or clear the search to see everything again.
              </CardDescription>
            </CardHeader>
          </Card>
        ) : null}

        {filteredSections.length > 0 ? (
          <div className="grid gap-6 lg:grid-cols-[280px_minmax(0,1fr)]">
            <aside className="lg:sticky lg:top-32 lg:self-start">
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
                  className="scroll-mt-36 overflow-hidden border-border/70 shadow-[0_20px_60px_-48px_color-mix(in_oklab,var(--foreground)_32%,transparent)]"
                >
                  <CardHeader className="gap-3 border-b border-border/60 bg-muted/15">
                    <div className="flex flex-wrap items-center gap-3">
                      <CardTitle className="text-2xl">{section.title}</CardTitle>
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
                        className="scroll-mt-40 space-y-5"
                      >
                        {subsectionIndex > 0 ? <SeparatorLine /> : null}
                        <div className="space-y-2">
                          <div className="flex flex-wrap items-center gap-3">
                            <h2 className="text-lg font-semibold">{subsection.title}</h2>
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
