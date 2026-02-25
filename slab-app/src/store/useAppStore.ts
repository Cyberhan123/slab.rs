import { create } from 'zustand';

interface AppState {
  isLoading: boolean;
  greetMessage: string;
  setIsLoading: (loading: boolean) => void;
  setGreetMessage: (message: string) => void;
  resetState: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  isLoading: false,
  greetMessage: '',
  setIsLoading: (loading) => set({ isLoading: loading }),
  setGreetMessage: (message) => set({ greetMessage: message }),
  resetState: () => set({ isLoading: false, greetMessage: '' }),
}));
