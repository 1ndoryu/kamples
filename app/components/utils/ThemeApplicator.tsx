'use client';

import { useEffect } from 'react';
import { useAppStore } from '@/store/useAppStore';

export function ThemeApplicator() {
  const theme = useAppStore((state) => state.theme);

  useEffect(() => {
    // Asegurarse de que documentElement exista (entorno de cliente)
    if (typeof window !== 'undefined') {
      document.documentElement.setAttribute('data-tema', theme);
    }
  }, [theme]);

  // Este efecto también maneja la carga inicial del tema desde el store (que a su vez lo carga de localStorage)
  // y lo aplica al HTML.

  return null; // No renderiza nada
}
