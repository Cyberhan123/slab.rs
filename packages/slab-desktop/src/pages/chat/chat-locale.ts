import { useMemo } from 'react';
import enUSX from '@ant-design/x/locale/en_US';
import zhCNX from '@ant-design/x/locale/zh_CN';
import enUSAntd from 'antd/locale/en_US';
import zhCNAntd from 'antd/locale/zh_CN';

import { getResolvedAppLanguage, useTranslation } from '@slab/i18n';

type ChatRuntimeLocale = {
  noData: string;
  requestAborted: string;
  requestFailed: string;
};

export function useChatLocale() {
  const { t } = useTranslation();
  const language = getResolvedAppLanguage();

  return useMemo(() => {
    const frameworkLocale = language === 'zh-CN'
      ? { ...zhCNAntd, ...zhCNX }
      : { ...enUSAntd, ...enUSX };

    return {
      ...frameworkLocale,
      noData: t('pages.chat.runtime.noData'),
      requestAborted: t('pages.chat.runtime.requestAborted'),
      requestFailed: t('pages.chat.runtime.requestFailed'),
    } as typeof frameworkLocale & ChatRuntimeLocale;
  }, [language, t]);
}
