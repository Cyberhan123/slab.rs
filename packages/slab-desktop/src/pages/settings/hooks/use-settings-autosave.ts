import { useEffect, useEffectEvent, useMemo, useRef, useState } from 'react';
import { toast } from 'sonner';

import {
  applyAppLanguagePreference,
  isAppLanguagePreference,
  useTranslation,
} from '@slab/i18n';
import api, { getErrorMessage } from '@/lib/api';

import type {
  DraftValue,
  FieldErrorState,
  FieldStatusState,
  SettingResponse,
} from '../types';
import { autoSaveDelay, buildRequestBody, extractStructuredError } from '../utils';

type UseSettingsAutosaveArgs = {
  propertyMap: Map<string, SettingResponse>;
  refetch: () => Promise<unknown>;
};

export function useSettingsAutosave({
  propertyMap,
  refetch,
}: UseSettingsAutosaveArgs) {
  const { t } = useTranslation();
  const [drafts, setDrafts] = useState<Record<string, DraftValue>>({});
  const [fieldErrors, setFieldErrors] = useState<Record<string, FieldErrorState>>({});
  const [fieldStatuses, setFieldStatuses] = useState<Record<string, FieldStatusState>>({});
  const [resettingPmid, setResettingPmid] = useState<string | null>(null);
  const autosaveTimersRef = useRef<Record<string, ReturnType<typeof setTimeout>>>({});
  const updateSettingMutation = api.useMutation('put', '/v1/settings/{pmid}');

  useEffect(() => {
    return () => {
      for (const timer of Object.values(autosaveTimersRef.current)) {
        clearTimeout(timer);
      }
    };
  }, []);

  const statusSummary = useMemo(() => {
    const values = Object.values(fieldStatuses);
    return {
      dirty: values.filter((value) => value.tone === 'dirty').length,
      saving: values.filter((value) => value.tone === 'saving').length,
      error: values.filter((value) => value.tone === 'error').length,
    };
  }, [fieldStatuses]);

  function clearFieldError(pmid: string) {
    setFieldErrors((current) => {
      if (!(pmid in current)) {
        return current;
      }

      const next = { ...current };
      delete next[pmid];
      return next;
    });
  }

  function setFieldStatus(pmid: string, nextState: FieldStatusState | null) {
    setFieldStatuses((current) => {
      if (!nextState) {
        if (!(pmid in current)) {
          return current;
        }

        const next = { ...current };
        delete next[pmid];
        return next;
      }

      return {
        ...current,
        [pmid]: nextState,
      };
    });
  }

  function clearAutosaveTimer(pmid: string) {
    const timer = autosaveTimersRef.current[pmid];
    if (!timer) {
      return;
    }

    clearTimeout(timer);
    delete autosaveTimersRef.current[pmid];
  }

  const saveDraft = useEffectEvent(async (pmid: string) => {
    const property = propertyMap.get(pmid);
    if (!property) {
      return;
    }

    const draftSnapshot = drafts[pmid];

    let body;
    try {
      body = buildRequestBody(property, draftSnapshot);
    } catch (error) {
      const message = getErrorMessage(error);
      setFieldErrors((current) => ({
        ...current,
        [pmid]: {
          message,
          path: '/',
        },
      }));
      setFieldStatus(pmid, {
        tone: 'error',
        message: t('pages.settings.autosave.needsAttention'),
      });
      return;
    }

    setFieldStatus(pmid, {
      tone: 'saving',
      message: t('pages.settings.autosave.savingChanges'),
    });

    try {
      await updateSettingMutation.mutateAsync({
        params: {
          path: {
            pmid,
          },
        },
        body,
      });

      await syncLanguagePreferenceSetting(
        pmid,
        body.op === 'set' ? body.value : property.schema.default_value,
      );

      let draftWasConsumed = false;
      setDrafts((current) => {
        if (current[pmid] !== draftSnapshot) {
          return current;
        }

        const next = { ...current };
        delete next[pmid];
        draftWasConsumed = true;
        return next;
      });

      if (draftWasConsumed) {
        clearFieldError(pmid);
        setFieldStatus(pmid, {
          tone: 'saved',
          message: t('pages.settings.autosave.savedAutomatically'),
        });
      } else {
        setFieldStatus(pmid, {
          tone: 'dirty',
          message: t('pages.settings.autosave.newEditsWaiting'),
        });
      }

      await refetch();
    } catch (error) {
      const structured = extractStructuredError(error);
      if (structured) {
        setFieldErrors((current) => ({
          ...current,
          [pmid]: structured,
        }));
      }

      setFieldStatus(pmid, {
        tone: 'error',
        message: structured?.message ?? getErrorMessage(error),
      });
    }
  });

  function scheduleAutosave(property: SettingResponse) {
    clearAutosaveTimer(property.pmid);
    autosaveTimersRef.current[property.pmid] = setTimeout(() => {
      delete autosaveTimersRef.current[property.pmid];
      void saveDraft(property.pmid);
    }, autoSaveDelay(property));
  }

  function setDraftValue(property: SettingResponse, value: DraftValue) {
    setDrafts((current) => ({
      ...current,
      [property.pmid]: value,
    }));
    clearFieldError(property.pmid);
    setFieldStatus(property.pmid, {
      tone: 'dirty',
      message: t('pages.settings.autosave.waitingAutoSave'),
    });
    scheduleAutosave(property);
  }

  async function resetSetting(property: SettingResponse) {
    clearAutosaveTimer(property.pmid);
    setResettingPmid(property.pmid);
    setFieldStatus(property.pmid, {
      tone: 'saving',
      message: t('pages.settings.autosave.resettingToDefault'),
    });

    try {
      await updateSettingMutation.mutateAsync({
        params: {
          path: {
            pmid: property.pmid,
          },
        },
        body: {
          op: 'unset',
        },
      });

      await syncLanguagePreferenceSetting(property.pmid, property.schema.default_value);

      setDrafts((current) => {
        if (!(property.pmid in current)) {
          return current;
        }

        const next = { ...current };
        delete next[property.pmid];
        return next;
      });
      clearFieldError(property.pmid);
      setFieldStatus(property.pmid, {
        tone: 'saved',
        message: t('pages.settings.autosave.restoredToDefault'),
      });
      await refetch();
      toast.success(
        t('pages.settings.autosave.resetToast', {
          label: property.label,
        }),
      );
    } catch (error) {
      const structured = extractStructuredError(error);
      if (structured) {
        setFieldErrors((current) => ({
          ...current,
          [property.pmid]: structured,
        }));
      }

      setFieldStatus(property.pmid, {
        tone: 'error',
        message: structured?.message ?? getErrorMessage(error),
      });
      toast.error(getErrorMessage(error));
    } finally {
      setResettingPmid(null);
    }
  }

  return {
    drafts,
    fieldErrors,
    fieldStatuses,
    resettingPmid,
    statusSummary,
    setDraftValue,
    resetSetting,
  };
}

async function syncLanguagePreferenceSetting(pmid: string, value: unknown) {
  if (pmid !== 'general.language') {
    return;
  }

  if (typeof value !== 'string' || !isAppLanguagePreference(value)) {
    return;
  }

  await applyAppLanguagePreference(value);
}
