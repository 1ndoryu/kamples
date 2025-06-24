'use client';

import { useEffect } from 'react';
import { useAppStore } from '@/store/useAppStore'; // Asumiendo alias @ para app/

export function StoreAuthInitializer() {
  const checkAuthStatusStore = useAppStore((state) => state.checkAuthStatusStore);
  const isLoading = useAppStore((state) => state.isLoading); // Para asegurar que solo se llama una vez o bajo ciertas condiciones

  useEffect(() => {
    // Solo llama si está en el estado de carga inicial o si no está autenticado aún.
    // La lógica interna de checkAuthStatusStore también previene múltiples llamadas si ya está autenticado.
    // Esto asegura que se ejecute al menos una vez al cargar la app.
    if (isLoading) { // isLoading se setea a true inicialmente en el store
        checkAuthStatusStore();
    }
  }, [checkAuthStatusStore, isLoading]);

  return null; // Este componente no renderiza nada
}
