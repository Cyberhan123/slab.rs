import { useTranslation } from "@slab/i18n";
import { describeSlabApiPermission } from "@slab/api/permissions";
import { ShieldCheck, X } from "lucide-react";

import { Button } from "@slab/components/button";
import { usePluginAuthorizationStore } from "@/store/usePluginAuthorizationStore";

/**
 * Settings surface for reviewing and revoking runtime plugin permission grants.
 * Grants live in the persisted `usePluginAuthorizationStore`; revoking one here
 * makes the next matching plugin request re-prompt the user.
 */
export function PluginPermissionsCard() {
  const { t } = useTranslation();
  const grants = usePluginAuthorizationStore((state) => state.grants);
  const revoke = usePluginAuthorizationStore((state) => state.revoke);

  const entries = Object.entries(grants).filter(([, perms]) => perms.length > 0);
  const hasAny = entries.length > 0;

  return (
    <section
      className="scroll-mt-8 rounded-[20px] border border-border/40 bg-[color:color-mix(in_oklab,var(--surface-soft)_70%,transparent)] p-6 md:p-8"
      data-testid="plugin-permissions-card"
    >
      <div className="space-y-2">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <ShieldCheck className="size-5 text-[color:var(--brand-teal)]" />
            <h2 className="text-lg font-bold tracking-tight text-foreground">
              {t("pages.plugins.permissions.management.title")}
            </h2>
          </div>
          {hasAny ? (
            <Button
              variant="quiet"
              size="sm"
              onClick={() => entries.forEach(([pluginId]) => revoke(pluginId))}
              data-testid="plugin-permissions-revoke-all"
            >
              {t("pages.plugins.permissions.management.revokeAll")}
            </Button>
          ) : null}
        </div>
        <p className="text-sm leading-7 text-muted-foreground">
          {t("pages.plugins.permissions.management.description")}
        </p>
      </div>

      <div className="mt-6 space-y-3">
        {!hasAny ? (
          <p className="rounded-xl border border-border/60 bg-background px-3 py-3 text-sm text-muted-foreground">
            {t("pages.plugins.permissions.management.empty")}
          </p>
        ) : (
          entries.map(([pluginId, permissions]) => (
            <div
              key={pluginId}
              className="rounded-xl border border-border/60 bg-background px-3 py-3"
              data-testid={`plugin-permissions-grant-${pluginId}`}
            >
              <p className="font-mono text-xs font-semibold text-foreground">{pluginId}</p>
              <ul className="mt-2 space-y-1.5">
                {permissions.map((permission) => {
                  const label = describeSlabApiPermission(permission);
                  return (
                    <li key={permission} className="flex items-center justify-between gap-2">
                      <span className="text-sm">
                        {label.title}
                        <span className="ml-1 text-xs text-muted-foreground">({permission})</span>
                      </span>
                      <Button
                        variant="quiet"
                        size="icon-xs"
                        title={t("pages.plugins.permissions.management.revoke")}
                        onClick={() => revoke(pluginId, permission)}
                        data-testid={`plugin-permissions-revoke-${pluginId}-${permission}`}
                      >
                        <X className="size-3.5" />
                      </Button>
                    </li>
                  );
                })}
              </ul>
            </div>
          ))
        )}
      </div>
    </section>
  );
}
