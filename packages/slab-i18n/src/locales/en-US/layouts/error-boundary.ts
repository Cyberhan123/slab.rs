export const errorBoundary = {
  details: 'Error details',
  global: {
    description: 'The desktop shell hit an unexpected error. Retry the app shell after checking the details.',
    retry: 'Try again',
    title: 'Something went wrong',
  },
  page: {
    back: 'Back',
    description: 'Only this page failed. The rest of Slab is still available from the sidebar.',
    retry: 'Retry page',
    title: 'Page crashed',
  },
} as const;
