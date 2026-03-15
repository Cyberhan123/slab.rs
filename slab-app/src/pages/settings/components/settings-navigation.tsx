import { ChevronRight, PanelLeft } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { ScrollArea } from '@/components/ui/scroll-area';
import { cn } from '@/lib/utils';

import type { SettingsSectionResponse } from '../types';
import { subsectionAnchorId, sectionAnchorId } from '../utils';

type SettingsNavigationProps = {
  activeTarget: string | null;
  dirtyCount: number;
  errorCount: number;
  savingCount: number;
  sections: SettingsSectionResponse[];
  onJump: (targetId: string) => void;
};

export function SettingsNavigation({
  activeTarget,
  dirtyCount,
  errorCount,
  savingCount,
  sections,
  onJump,
}: SettingsNavigationProps) {
  return (
    <Card className="overflow-hidden border-border/70 shadow-[0_20px_60px_-48px_color-mix(in_oklab,var(--foreground)_32%,transparent)]">
      <CardHeader className="gap-3 border-b border-border/60 bg-muted/15">
        <div className="flex items-center gap-2">
          <PanelLeft className="h-4 w-4 text-muted-foreground" />
          <CardTitle className="text-base">Navigation</CardTitle>
        </div>
        <CardDescription className="text-sm leading-6">
          Jump directly to the section you want to edit.
        </CardDescription>
        <div className="flex flex-wrap gap-2">
          <Badge variant="outline">{sections.length} sections</Badge>
          {dirtyCount > 0 ? <Badge variant="secondary">{dirtyCount} pending</Badge> : null}
          {savingCount > 0 ? <Badge variant="secondary">{savingCount} saving</Badge> : null}
          {errorCount > 0 ? <Badge variant="destructive">{errorCount} errors</Badge> : null}
        </div>
      </CardHeader>
      <CardContent className="p-0">
        <ScrollArea className="max-h-[calc(100vh-16rem)]">
          <div className="space-y-3 p-4">
            {sections.map((section) => {
              const sectionTargetId = sectionAnchorId(section.id);

              return (
                <div key={section.id} className="space-y-1">
                  <Button
                    variant="ghost"
                    className={cn(
                      'h-auto w-full justify-start px-3 py-2 text-left',
                      activeTarget === sectionTargetId && 'bg-accent text-accent-foreground',
                    )}
                    onClick={() => onJump(sectionTargetId)}
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      <ChevronRight className="h-4 w-4 shrink-0 text-muted-foreground" />
                      <div className="min-w-0">
                        <p className="truncate font-medium">{section.title}</p>
                        <p className="text-xs text-muted-foreground">{section.id}</p>
                      </div>
                    </div>
                  </Button>

                  <div className="ml-4 space-y-1 border-l border-border/60 pl-3">
                    {section.subsections.map((subsection) => {
                      const subsectionTargetId = subsectionAnchorId(section.id, subsection.id);

                      return (
                        <Button
                          key={subsection.id}
                          variant="ghost"
                          size="sm"
                          className={cn(
                            'h-auto w-full justify-start px-2 py-2 text-left',
                            activeTarget === subsectionTargetId &&
                              'bg-accent text-accent-foreground',
                          )}
                          onClick={() => onJump(subsectionTargetId)}
                        >
                          <div className="min-w-0">
                            <p className="truncate">{subsection.title}</p>
                            <p className="text-xs text-muted-foreground">
                              {subsection.properties.length} fields
                            </p>
                          </div>
                        </Button>
                      );
                    })}
                  </div>
                </div>
              );
            })}
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  );
}
