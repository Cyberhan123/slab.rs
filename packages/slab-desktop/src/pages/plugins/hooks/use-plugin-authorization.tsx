import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "@slab/i18n";
import { Button } from "@slab/components/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@slab/components/dialog";
import { describeSlabApiPermission } from "@slab/plugin-sdk";

import { usePluginAuthorizationStore } from "@/store/usePluginAuthorizationStore";

type AuthorizeContext = {
  method: string;
  path: string;
};

type PendingRequest = {
  permission: string;
  context: AuthorizeContext;
  resolve: (allowed: boolean) => void;
};

/**
 * Runtime first-reject gate for plugin Slab API calls. `authorize(permission, ctx)`
 * resolves immediately when the user has already granted `(pluginId × permission)`;
 * otherwise it opens a modal describing the request and waits for Allow/Deny. Allow
 * records the grant (persisted) so subsequent calls do not re-prompt; Deny rejects
 * without ever reaching the backend. The backend permission check remains the final
 * authority — this only avoids surprise prompts and gives the user a clear choice.
 *
 * Prompts are serialized through a queue so that two distinct unauthorized
 * permissions requested in quick succession each get their own resolution (a plugin
 * often fires models:read then chat:complete on load). Without this, the second
 * request would overwrite the first prompt and leave its promise — and the plugin's
 * api.response — hanging forever.
 */
export function usePluginAuthorization(pluginId: string, pluginName: string) {
  const { t } = useTranslation();
  const isAuthorized = usePluginAuthorizationStore((state) => state.isAuthorized);
  const grant = usePluginAuthorizationStore((state) => state.grant);

  const queueRef = useRef<PendingRequest[]>([]);
  const [current, setCurrent] = useState<PendingRequest | null>(null);
  const currentRef = useRef<PendingRequest | null>(null);
  currentRef.current = current;

  const showNext = useCallback(() => {
    const next = queueRef.current.shift() ?? null;
    currentRef.current = next;
    setCurrent(next);
  }, []);

  const settle = useCallback(
    (allowed: boolean) => {
      const pending = currentRef.current;
      if (pending) {
        if (allowed) {
          grant(pluginId, pending.permission);
        }
        pending.resolve(allowed);
      }
      showNext();
    },
    [grant, pluginId, showNext],
  );

  const authorize = useCallback(
    (permission: string, context: AuthorizeContext): Promise<boolean> => {
      if (isAuthorized(pluginId, permission)) {
        return Promise.resolve(true);
      }
      return new Promise<boolean>((resolve) => {
        const request: PendingRequest = { permission, context, resolve };
        if (currentRef.current) {
          // A prompt is already open: queue behind it so each request resolves.
          queueRef.current.push(request);
        } else {
          currentRef.current = request;
          setCurrent(request);
        }
      });
    },
    [isAuthorized, pluginId],
  );

  // If the owning view unmounts mid-prompt, fail closed (deny) for the pending
  // request so awaiting callers never hang.
  useEffect(() => {
    return () => {
      const pending = currentRef.current;
      queueRef.current.forEach((queued) => queued.resolve(false));
      queueRef.current = [];
      pending?.resolve(false);
    };
  }, []);

  const label = describeSlabApiPermission(current?.permission ?? "");

  const prompt: ReactNode = (
    <Dialog open={current !== null} onOpenChange={(open) => !open && settle(false)}>
      <DialogContent className="max-w-md" data-testid="plugin-authorization-dialog">
        <DialogHeader>
          <DialogTitle>{t("pages.plugins.permissions.prompt.title")}</DialogTitle>
          <DialogDescription>
            {t("pages.plugins.permissions.prompt.description", {
              name: pluginName,
              method: current?.context.method ?? "",
              path: current?.context.path ?? "",
              permission: current?.permission ?? "",
            })}
          </DialogDescription>
        </DialogHeader>
        <div className="rounded-xl border border-border/60 bg-background px-3 py-2 text-xs">
          <span className="font-medium">{label.title}</span>
          <span className="text-muted-foreground"> — {label.description}</span>
        </div>
        <DialogFooter>
          <Button variant="quiet" onClick={() => settle(false)} data-testid="plugin-authorization-deny">
            {t("pages.plugins.permissions.prompt.deny")}
          </Button>
          <Button variant="cta" onClick={() => settle(true)} data-testid="plugin-authorization-allow">
            {t("pages.plugins.permissions.prompt.allow")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );

  return { authorize, prompt };
}
