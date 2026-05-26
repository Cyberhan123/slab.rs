import { theme } from 'antd';
import { useMemo } from 'react';

export const useMarkdownTheme = () => {
  const token = theme.useToken();

  const isLightMode = useMemo(() => {
    return token?.theme?.id === 0;
  }, [token]);

  const className = useMemo(() => {
    return isLightMode ? 'x-markdown-light' : 'x-markdown-dark';
  }, [isLightMode]);

  return [className];
};
