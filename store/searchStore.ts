'use client';
import { create } from 'zustand';

interface SearchState {
  searchTerm: string;
  setSearchTerm: (q: string) => void;
}

export const useSearchStore = create<SearchState>((set) => ({
  searchTerm: '',
  setSearchTerm: (q) => set({ searchTerm: q }),
})); 