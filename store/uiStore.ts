'use client';
import { create } from 'zustand';

export type ModalType = 'login' | 'publicar' | 'perfil' | null;

interface UiState {
  modalOpen: ModalType;
  openModal: (id: Exclude<ModalType, null>) => void;
  closeModal: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  modalOpen: null,
  openModal: (id) => set({ modalOpen: id }),
  closeModal: () => set({ modalOpen: null }),
})); 