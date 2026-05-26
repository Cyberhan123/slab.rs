import { useMemo } from 'react';
import enUSX from '@ant-design/x/locale/en_US';
import zhCNX from '@ant-design/x/locale/zh_CN';
import enUSAntd from 'antd/locale/en_US';
import zhCNAntd from 'antd/locale/zh_CN';

import { getResolvedAppLanguage, useTranslation } from '@slab/i18n';

type AssistantRuntimeLocale = {
  approvalFailed: string;
  approvalNotDelivered: string;
  eventStreamLagged: string;
  interruptFailed: string;
  noData: string;
  requestAborted: string;
  requestFailed: string;
};

export function useAssistantLocale() {
  const { t } = useTranslation();
  const language = getResolvedAppLanguage();

  return useMemo(() => {
    const frameworkLocale = language === 'zh-CN'
      ? { ...zhCNAntd, ...zhCNX }
      : { ...enUSAntd, ...enUSX };

    return {
      ...frameworkLocale,
      approvalFailed: t('pages.assistant.toast.approvalFailed'),
      approvalNotDelivered: t('pages.assistant.toast.approvalNotDelivered'),
      eventStreamLagged: t('pages.assistant.timeline.lagged'),
      interruptFailed: t('pages.assistant.toast.interruptFailed'),
      noData: t('pages.assistant.runtime.noData'),
      requestAborted: t('pages.assistant.runtime.requestAborted'),
      requestFailed: t('pages.assistant.runtime.requestFailed'),
    } as typeof frameworkLocale & AssistantRuntimeLocale;
  }, [language, t]);
}
