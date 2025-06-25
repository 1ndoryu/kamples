import { create } from 'zustand';
import { Usuario } from '@/domain/entities/Usuario';
import { secureFetch } from './helpers/secureFetch';

interface AuthState {
    usuario: Usuario | null;
    cargando: boolean;
    setCargando: (cargando: boolean) => void;
    login: (nombreUsuario: string, clave: string) => Promise<{ exito: boolean; error?: string }>;
    logout: () => Promise<{ exito: boolean; error?: string }>;
    verificarSesion: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set, get) => ({
    usuario: null,
    cargando: true,
    setCargando: (cargando: boolean) => set({ cargando }),

    verificarSesion: async () => {
        const { setCargando } = get()
        const { error, data } = await secureFetch<{ usuario: Usuario }>('/api/auth/sesion', { method: 'GET' }, setCargando);
        if (error) {
            console.log(`Ha ocurrido un error al verificar la sesión ${error}`);
            return;
        }
        
        set({ usuario: data?.usuario });
    },

    login: async (nombreUsuario: string, clave: string) => {
        const { setCargando } = get()
        const { error, data } = await secureFetch<{ usuario: Usuario }>(
            '/api/auth/login',
            {
                method: 'POST',
                body: JSON.stringify({ nombreUsuario, clave }),
            }, setCargando);
        if (error) {
            console.log(error);
            return { exito: false, error };
        }
        set({ usuario: data?.usuario });
        return { exito: true };
    },

    logout: async () => {
        const { setCargando } = get()
        const { error } = await secureFetch('/api/auth/logout', { method: 'POST' }, setCargando);

        if (error) {
            console.log(error);
            return { exito: false, error };
        }

        set({ usuario: null });
        return { exito: true };
    },
}));