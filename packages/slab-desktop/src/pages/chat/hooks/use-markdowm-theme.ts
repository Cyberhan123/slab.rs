import { theme } from 'antd';
import { useMemo } from 'react';

const splitIntoChunks = (str: string, chunkSize: number) => {
  const chunks = [];
  for (let i = 0; i < str.length; i += chunkSize) {
    chunks.push(str.slice(i, i + chunkSize));
  }
  return chunks;
};

export const mockFetch = async (fullContent: string, onFinish?: () => void) => {
  const chunks = splitIntoChunks(fullContent, 7);
  const response = new Response(
    new ReadableStream({
      async start(controller) {
        try {
          await new Promise((resolve) => setTimeout(resolve, 100));
          for (const chunk of chunks) {
            // eslint-disable-next-line no-await-in-loop
            await new Promise((resolve) => setTimeout(resolve, 100));
            if (!controller.desiredSize) {
            
              return;
            }
            controller.enqueue(new TextEncoder().encode(chunk));
          }
          onFinish?.();
          controller.close();
        } catch (error) {
          console.log(error);
        }
      },
    }),
    {
      headers: {
        'Content-Type': 'application/x-ndjson',
      },
    },
  );

  return response;
};

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