import { useEffect, useRef } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import {
  applyAppLanguagePreference,
  isAppLanguagePreference,
} from "@slab/i18n";
import api from "@slab/api";

/**
 * Shell-agnostic app-level guards, extracted from the desktop App so the
 * web/h5 shells can mount them without pulling the desktop-only syncs
 * (workspace redirect, plugin theme bridge, monaco preload) into their
 * bundles.
 */

/**
 * Checks whether the one-time setup wizard has been completed the first time
 * the shell needs it. Redirects to /setup only when the server responds and
 * reports `initialized: false`.
 *
 * The desktop host now spawns `slab-server` asynchronously, so transient
 * transport errors during boot should not be treated as a setup signal.
 */
export function SetupGuard() {
  const navigate = useNavigate();
  const location = useLocation();
  const isSetupRoute = location.pathname === "/setup";

  const { data: setupStatus, refetch: refetchSetupStatus } = api.useQuery(
    "get",
    "/v1/setup/status",
    undefined,
    {
      enabled: !isSetupRoute,
      staleTime: 0,
      refetchOnMount: "always",
      refetchOnReconnect: true,
      refetchOnWindowFocus: true,
      // The setup guard is a redirect gate; boot-time transport failures should
      // be observed on the next explicit probe instead of retried into navigation.
      retry: false,
    }
  );

  useEffect(() => {
    if (isSetupRoute) {
      return;
    }

    if (setupStatus?.initialized === false) {
      navigate("/setup", { replace: true });
    }
  }, [isSetupRoute, navigate, setupStatus?.initialized]);

  useEffect(() => {
    if (isSetupRoute) {
      return;
    }

    void refetchSetupStatus();
  }, [isSetupRoute, location.pathname, refetchSetupStatus]);

  return null;
}

/** Applies the persisted `general.language` preference to the running i18n instance. */
export function AppLanguageSync() {
  const lastAppliedPreferenceRef = useRef<string | null>(null);
  const { data } = api.useQuery(
    "get",
    "/v1/settings/{pmid}",
    {
      params: {
        path: {
          pmid: "general.language",
        },
      },
    },
    {
      staleTime: Number.POSITIVE_INFINITY,
      refetchOnMount: false,
      refetchOnReconnect: true,
      refetchOnWindowFocus: true,
    }
  );

  useEffect(() => {
    const preference = data?.effective_value;
    if (typeof preference !== "string" || !isAppLanguagePreference(preference)) {
      return;
    }

    if (lastAppliedPreferenceRef.current === preference) {
      return;
    }

    lastAppliedPreferenceRef.current = preference;
    void applyAppLanguagePreference(preference);
  }, [data?.effective_value]);

  return null;
}
