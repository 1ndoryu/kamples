'use client';

import {createContext, useState, useContext, ReactNode, useEffect} from 'react';
import {useRouter} from 'next/navigation';
import { Usuario } from '@/domain/entities/Usuario';
import { useFetch } from '@/hooks/useFetch';

interface AuthContextType {
    usuario: Usuario | null;
    login: (nombreUsuario: string, clave: string) => Promise<{exito: boolean; error?: string}>;
    logout: () => void;
    cargando: boolean;
}

export const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({children}: {children: ReactNode}) {
    const [usuario, setUsuario] = useState<Usuario | null>(null);
    const [cargando, setCargando] = useState<boolean>(true);
    const router = useRouter();

    useEffect(() => {
        const verificarSesion = async () => {
            const { data: usuario, error } = await useFetch('/api/auth/sesion', { method: 'GET' }, setCargando);
            if (error) {
                console.error('Error al verificar la sesión:', error);
                return;
            }

            if (usuario) {
                setUsuario(usuario);
            }
        };
        verificarSesion();
    }, []);

    const login = async (nombreUsuario: string, clave: string) => {
        setCargando(true);
        const { data, error } = await useFetch('/api/auth/login', { 
            method: 'POST', 
            body: JSON.stringify({nombreUsuario, clave}) }, setCargando);

            if (error) {
                return { exito: false, error: 'Error al iniciar sesión' };
            }

            setUsuario(data.usuario);
            router.push('/');
            return { exito: true };
    };

    const logout = async () => {
        const { error } = await useFetch('/api/auth/logout', { method: 'POST' }, setCargando);
        if (error) {
            console.error('Error al cerrar sesión:', error);
            return
        }

        setUsuario(null);
        router.push('/');
    };

    return <AuthContext.Provider value={{usuario, login, logout, cargando}}>{children}</AuthContext.Provider>;
}

export function useAuth() {
    const context = useContext(AuthContext);
    if (context === undefined) {
        throw new Error('useAuth debe ser usado dentro de un AuthProvider');
    }
    return context;
}
