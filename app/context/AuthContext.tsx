// app/context/AuthContext.tsx
'use client';

import {createContext, useState, useContext, ReactNode, useEffect} from 'react';
import {useRouter} from 'next/navigation';

interface Usuario {
    id: number;
    nombreusuario: string;
    nombremostrado: string;
    correoelectronico: string;
    rol: string;
    imagen_perfil?: string; // CAMBIO: Se añade la imagen de perfil opcional.
}

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
            try {
                const respuesta = await fetch('/api/auth/sesion');
                if (respuesta.ok) {
                    const {usuario} = await respuesta.json();
                    setUsuario(usuario);
                }
            } catch (error) {
                console.error('No hay sesión activa', error);
            } finally {
                setCargando(false);
            }
        };
        verificarSesion();
    }, []);

    const login = async (nombreUsuario: string, clave: string) => {
        setCargando(true);
        try {
            const respuesta = await fetch('/api/auth/login', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({nombreUsuario, clave})
            });

            const datos = await respuesta.json();

            if (!respuesta.ok) {
                return {exito: false, error: datos.error};
            }

            setUsuario(datos.usuario);
            router.push('/'); // Redirige a la página principal tras el login
            return {exito: true};
        } catch (error) {
            console.error('Función login:', error);
            return {exito: false, error: 'Error de conexión con el servidor.'};
        } finally {
            setCargando(false);
        }
    };

    const logout = async () => {
        try {
            await fetch('/api/auth/logout', {method: 'POST'});
        } catch (error) {
            console.error('Error al cerrar sesión:', error);
        } finally {
            setUsuario(null);
            router.push('/'); // Llevamos al usuario a la página principal
        }
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
