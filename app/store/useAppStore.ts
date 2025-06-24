import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import { Sample } from '../types/sample';
import { obtenerSamples, obtenerSamplePorSlug } from '../services/swordApi'; // Importar funciones de la API

// Define los tipos para el estado de autenticación
interface AuthState {
  user: User | null;
  token: string | null;
  isLoading: boolean;
  error: string | null;
  isAuthenticated: boolean;
  setUser: (user: User | null, token: string | null) => void; // Token podría no ser necesario si la API usa cookies de sesión
  setAuthLoading: (isLoading: boolean) => void; // Renombrado para claridad
  setAuthError: (error: string | null) => void; // Renombrado para claridad
  loginStore: (nombreUsuario: string, clave: string) => Promise<{ exito: boolean; error?: string }>;
  logoutStore: () => Promise<void>;
  checkAuthStatusStore: () => Promise<void>;
}

// Define el tipo para el usuario, adaptado de AuthContext.tsx
interface User {
  id: number;
  nombreusuario: string; // Anteriormente username
  nombremostrado: string;
  correoelectronico: string; // Anteriormente email
  rol: string;
  imagen_perfil?: string;
}

// Define los tipos para el estado del tema
type Theme = 'claro' | 'oscuro'; // Ajustado a los valores de TemaContext
interface ThemeState {
  theme: Theme;
  toggleTheme: () => void;
  setTheme: (theme: Theme) => void;
}

// Define los tipos para el estado de los Samples
interface SamplesState {
  samples: Sample[];
  selectedSample: Sample | null;
  isLoadingSamples: boolean;
  errorSamples: string | null;
  setSamples: (samples: Sample[]) => void;
  setSelectedSample: (sample: Sample | null) => void;
  addSample: (sample: Sample) => void;
  fetchSamplesStore: () => Promise<void>;
  fetchSampleBySlugStore: (slug: string) => Promise<Sample | null>;
  uploadSampleStore: (formData: any) => Promise<{ success: boolean; sample?: Sample; error?: string }>; // formData podría ser un tipo más específico
}

// Combina todos los slices del estado
type AppState = AuthState & ThemeState & SamplesState;

export const useAppStore = create<AppState>()(
  persist(
    (set, get) => ({
      // Estado y acciones de Autenticación
      user: null,
      token: null, // Asumimos que el token se maneja con cookies HttpOnly por el backend si no se pasa explícitamente
      isLoading: true, // Inicia como true hasta que checkAuthStatusStore se ejecute
      error: null,
      isAuthenticated: false,
      setUser: (user, token) => { // Token podría ser opcional o no usado aquí si es HttpOnly
        set({ user, token: token || null, isAuthenticated: !!user, isLoading: false, error: null });
      },
      setAuthLoading: (isLoading) => set({ isLoading }),
      setAuthError: (error) => set({ error, isLoading: false }),

      loginStore: async (nombreUsuario, clave) => {
        set({ isLoading: true, error: null });
        try {
          const respuesta = await fetch('/api/auth/login', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ nombreUsuario, clave }),
          });
          const datos = await respuesta.json();
          if (!respuesta.ok) {
            set({ error: datos.error || 'Error en el login', isLoading: false, isAuthenticated: false });
            return { exito: false, error: datos.error || 'Error en el login' };
          }
          // Asumimos que 'datos.usuario' es el objeto User y 'datos.token' (si existe) es el token
          set({ user: datos.usuario, token: datos.token || null, isAuthenticated: true, isLoading: false, error: null });
          return { exito: true };
        } catch (error: any) {
          console.error('Error en loginStore:', error);
          const errorMessage = error.message || 'Error de conexión con el servidor.';
          set({ error: errorMessage, isLoading: false, isAuthenticated: false });
          return { exito: false, error: errorMessage };
        }
      },

      logoutStore: async () => {
        set({ isLoading: true });
        try {
          await fetch('/api/auth/logout', { method: 'POST' });
        } catch (error: any) {
          // Incluso si el logout de API falla, limpiamos el estado local
          console.error('Error en logoutStore API call:', error);
        } finally {
          set({ user: null, token: null, isAuthenticated: false, isLoading: false, error: null });
        }
      },

      checkAuthStatusStore: async () => {
        // Evita ejecutar si ya está autenticado y no es la carga inicial (isLoading es true al inicio)
        if (get().isAuthenticated && !get().isLoading) return;

        // Si isLoading es false y no estamos autenticados, podría ser una ejecución posterior, entonces ponemos loading.
        // Pero si es la carga inicial (isLoading es true por defecto), no necesitamos setearlo de nuevo.
        if (!get().isLoading) {
            set({ isLoading: true, error: null });
        }

        try {
          const respuesta = await fetch('/api/auth/sesion');
          if (respuesta.ok) {
            const { usuario, token } = await respuesta.json(); // Asume que la API devuelve usuario y opcionalmente token
            set({ user: usuario, token: token || null, isAuthenticated: true, isLoading: false, error: null });
          } else {
            set({ user: null, token: null, isAuthenticated: false, isLoading: false, error: null });
          }
        } catch (error) {
          console.error('Error en checkAuthStatusStore:', error);
          set({ user: null, token: null, isAuthenticated: false, isLoading: false, error: 'Error al verificar sesión' });
        }
      },

      // Estado y acciones del Tema
      theme: 'oscuro', // Valor inicial por defecto, igual que en TemaProvider y data-tema en html
      toggleTheme: () =>
        set((state) => ({ theme: state.theme === 'claro' ? 'oscuro' : 'claro' })),
      setTheme: (theme) => set({ theme }),

      // Estado y acciones de Samples
      samples: [],
      selectedSample: null,
      isLoadingSamples: false,
      errorSamples: null,
      setSamples: (samples) => set({ samples, isLoadingSamples: false, errorSamples: null }),
      setSelectedSample: (sample) => set({ selectedSample: sample }),
      addSample: (sample) => set((state) => ({ samples: [sample, ...state.samples] })), // Añade al principio

      fetchSamplesStore: async () => {
        if (get().isLoadingSamples) return;
        set({ isLoadingSamples: true, errorSamples: null });
        try {
          const fetchedSamples = await obtenerSamples(); // Directamente de swordApi
          set({ samples: fetchedSamples, isLoadingSamples: false });
        } catch (error: any) {
          console.error('Error en fetchSamplesStore:', error);
          const message = error instanceof Error ? error.message : 'Error al cargar los samples';
          set({ errorSamples: message, isLoadingSamples: false });
        }
      },

      fetchSampleBySlugStore: async (slug: string) => {
        set({ isLoadingSamples: true, errorSamples: null, selectedSample: null });
        try {
          const fetchedSample = await obtenerSamplePorSlug(slug); // Directamente de swordApi
          if (fetchedSample) {
            set({ selectedSample: fetchedSample, isLoadingSamples: false });
            // Opcional: actualizar la lista de samples si el sample no está o es diferente
            // get().addSample(fetchedSample); // o una lógica más compleja de actualización
          } else {
            set({ errorSamples: `Sample con slug "${slug}" no encontrado.`, isLoadingSamples: false });
          }
          return fetchedSample;
        } catch (error: any) {
          console.error(`Error en fetchSampleBySlugStore (slug: ${slug}):`, error);
          const message = error instanceof Error ? error.message : `Error al cargar el sample con slug ${slug}`;
          set({ errorSamples: message, isLoadingSamples: false });
          return null;
        }
      },

      uploadSampleStore: async (formData: FormData) => { // Ahora espera un objeto FormData
        set({ isLoadingSamples: true, errorSamples: null });
        try {
          const response = await fetch('/api/samples', { // Llama a la nueva ruta API
            method: 'POST',
            // NO establecer Content-Type manualmente para FormData, el navegador lo hace.
            body: formData,
          });

          const result = await response.json();

          if (!response.ok) {
            const errorMessage = result.error || `Error al subir el sample (HTTP ${response.status})`;
            set({ errorSamples: errorMessage, isLoadingSamples: false });
            return { success: false, error: errorMessage };
          }

          get().addSample(result as Sample);
          set({ isLoadingSamples: false, errorSamples: null });
          return { success: true, sample: result as Sample };

        } catch (error: any) {
          console.error('Error en uploadSampleStore:', error);
          const message = error.message || 'Error de conexión al subir el sample.';
          set({ errorSamples: message, isLoadingSamples: false });
          return { success: false, error: message };
        }
      },
    }),
    {
      name: 'app-storage', // nombre de la clave en localStorage
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        user: state.user,
        token: state.token,
        isAuthenticated: state.isAuthenticated,
        theme: state.theme,
      }),
    }
  )
);

export type { User, Theme }; // Sample ya se importa y se usa

// Selectors
export const useCurrentUser = () => useAppStore((state) => state.user);
export const useIsAuthenticated = () => useAppStore((state) => state.isAuthenticated);
export const useAuthLoading = () => useAppStore((state) => state.isLoading);
export const useAuthError = () => useAppStore((state) => state.error);

export const useCurrentTheme = () => useAppStore((state) => state.theme);
export const useThemeActions = () => {
  const setTheme = useAppStore((state) => state.setTheme);
  const toggleTheme = useAppStore((state) => state.toggleTheme);
  return { setTheme, toggleTheme };
};

export const useAllSamples = () => useAppStore((state) => state.samples);
export const useSelectedSample = () => useAppStore((state) => state.selectedSample);
export const useSamplesLoading = () => useAppStore((state) => state.isLoadingSamples);
export const useSamplesError = () => useAppStore((state) => state.errorSamples);
// export const useFetchSamples = () => useAppStore((state) => state.fetchSamples); // Descomentar si se implementa fetchSamples

console.log('Zustand store useAppStore.ts creado con importación de Sample.');
