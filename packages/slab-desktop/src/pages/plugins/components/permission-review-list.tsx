import { useTranslation } from "@slab/i18n";
import {
  describeSlabApiPermission,
  isKnownSlabApiPermission,
  type SlabApiPermissionSeverity,
} from "@slab/api/permissions";
import { AlertTriangle } from "lucide-react";

import type { PluginManifestPreview } from "../lib/plugin-manifest-preview";

const SEVERITY_BADGE_CLASS: Record<SlabApiPermissionSeverity, string> = {
  low: "bg-emerald-500/12 text-emerald-600 dark:text-emerald-300",
  medium: "bg-amber-500/12 text-amber-700 dark:text-amber-300",
  high: "bg-rose-500/12 text-rose-600 dark:text-rose-300",
};

type PermissionReviewListProps = {
  preview: PluginManifestPreview;
};

export function PermissionReviewList({ preview }: PermissionReviewListProps) {
  const { t } = useTranslation();
  const { permissions } = preview;

  const slabApiEntries = permissions.slabApi.map((permission) => ({
    permission,
    label: describeSlabApiPermission(permission),
    known: isKnownSlabApiPermission(permission),
  }));
  const hasAny =
    slabApiEntries.length > 0 ||
    permissions.filesRead.length > 0 ||
    permissions.filesWrite.length > 0 ||
    permissions.agent.length > 0 ||
    permissions.lsp.length > 0 ||
    permissions.networkMode !== null;

  return (
    <div className="space-y-3">
      <div>
        <p className="text-sm font-semibold">{t("pages.plugins.permissions.reviewTitle")}</p>
        <p className="mt-1 text-xs text-muted-foreground">
          {t("pages.plugins.permissions.reviewDescription")}
        </p>
      </div>

      {!hasAny ? (
        <p className="rounded-xl border border-border/60 bg-background px-3 py-2 text-xs text-muted-foreground">
          {t("pages.plugins.permissions.none")}
        </p>
      ) : null}

      {slabApiEntries.length > 0 ? (
        <PermissionGroup title={t("pages.plugins.permissions.group.slabApi")}>
          {slabApiEntries.map(({ permission, label, known }) => (
            <div
              key={permission}
              className="rounded-xl border border-border/60 bg-background px-3 py-2"
              data-testid={`plugin-permission-${permission}`}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="text-sm font-medium">{label.title}</span>
                <span
                  className={`rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase ${SEVERITY_BADGE_CLASS[label.severity]}`}
                >
                  {t(`pages.plugins.permissions.severity.${label.severity}`)}
                </span>
              </div>
              <p className="mt-1 text-xs text-muted-foreground">{label.description}</p>
              {!known ? (
                <p className="mt-1 flex items-center gap-1 text-xs text-rose-600 dark:text-rose-300">
                  <AlertTriangle className="size-3" />
                  {t("pages.plugins.permissions.unknownWarning")}
                </p>
              ) : null}
            </div>
          ))}
        </PermissionGroup>
      ) : null}

      {permissions.filesRead.length > 0 || permissions.filesWrite.length > 0 ? (
        <PermissionGroup title={t("pages.plugins.permissions.group.files")}>
          <PermissionChips
            items={[
              ...permissions.filesRead.map((scope) => `read: ${scope}`),
              ...permissions.filesWrite.map((scope) => `write: ${scope}`),
            ]}
          />
        </PermissionGroup>
      ) : null}

      {permissions.networkMode ? (
        <PermissionGroup title={t("pages.plugins.permissions.group.network")}>
          <PermissionChips
            items={[
              t(`pages.plugins.permissions.networkMode.${permissions.networkMode}`),
              ...permissions.networkHosts.map((host) => `host: ${host}`),
            ]}
          />
        </PermissionGroup>
      ) : null}

      {permissions.agent.length > 0 ? (
        <PermissionGroup title={t("pages.plugins.permissions.group.agent")}>
          <PermissionChips items={permissions.agent} />
        </PermissionGroup>
      ) : null}

      {permissions.lsp.length > 0 ? (
        <PermissionGroup title={t("pages.plugins.permissions.group.lsp")}>
          <PermissionChips items={permissions.lsp} />
        </PermissionGroup>
      ) : null}
    </div>
  );
}

function PermissionGroup({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-1.5">
      <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
        {title}
      </p>
      <div className="space-y-1.5">{children}</div>
    </div>
  );
}

function PermissionChips({ items }: { items: string[] }) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {items.map((item) => (
        <span
          key={item}
          className="rounded-full bg-muted px-2 py-0.5 font-mono text-[11px] text-muted-foreground"
        >
          {item}
        </span>
      ))}
    </div>
  );
}
