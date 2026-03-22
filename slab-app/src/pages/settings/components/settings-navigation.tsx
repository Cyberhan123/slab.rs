import {
  Cloud,
  Cpu,
  Download,
  SlidersHorizontal,
  Sparkles,
  type LucideIcon,
} from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

import type { SettingsSectionResponse } from '../types';
import { countSectionProperties } from '../utils';

type SettingsNavigationProps = {
  activeSectionId: string | null;
  sections: SettingsSectionResponse[];
  onSelectSection: (sectionId: string) => void;
};

const sectionIcons: Record<string, LucideIcon> = {
  cloud: Cloud,
  diffusion: Sparkles,
  runtime: Cpu,
  setup: Download,
};

export function SettingsNavigation({
  activeSectionId,
  sections,
  onSelectSection,
}: SettingsNavigationProps) {
  return (
    <div className="flex h-full flex-col px-4 py-5 lg:px-5 lg:py-6">
      <nav className="flex flex-col gap-1.5 overflow-y-auto pr-1">
        {sections.map((section) => {
          const SectionIcon = sectionIcons[section.id] ?? SlidersHorizontal;
          const isActiveSection = section.id === activeSectionId;

          return (
            <button
              key={section.id}
              type="button"
              onClick={() => onSelectSection(section.id)}
              className={cn(
                'flex w-full items-center gap-3 rounded-[16px] px-3 py-2.5 text-left transition-colors',
                isActiveSection
                  ? 'bg-white text-teal-700 shadow-[0_1px_2px_rgba(15,23,42,0.06)]'
                  : 'text-slate-600 hover:bg-white/80 hover:text-slate-900',
              )}
            >
              <span
                className={cn(
                  'flex size-8 shrink-0 items-center justify-center rounded-[12px]',
                  isActiveSection ? 'bg-teal-50 text-teal-600' : 'bg-transparent text-slate-400',
                )}
              >
                <SectionIcon className="size-4" />
              </span>

              <span className="min-w-0 flex-1">
                <span
                  className={cn(
                    'block truncate text-sm',
                    isActiveSection ? 'font-semibold text-teal-700' : 'font-medium',
                  )}
                >
                  {section.title}
                </span>
              </span>

              {isActiveSection ? (
                <Badge
                  variant="counter"
                  className="rounded-full border-white/80 bg-slate-100 px-2 py-0.5 text-[10px] font-bold text-slate-600 shadow-none"
                >
                  {countSectionProperties(section)}
                </Badge>
              ) : null}
            </button>
          );
        })}
      </nav>
    </div>
  );
}
