// app/context/TemaContext.tsx
'use client';

import { createContext, useState, useEffect, useContext, ReactNode, useCallback } from 'react';

type Tema = 'claro' | 'oscuro';

interface TemaContextType {
    tema: Tema;
    cambiarTema: () => void;
}

export const TemaContext = createContext<TemaContextType | undefined>(undefined);

export function TemaProvider({ children }: { children: ReactNode }) {
    const [tema, setTema] = useState<Tema>('oscuro'); // Valor por defecto que coincide con el servidor

    // Este efecto se ejecuta solo una vez en el cliente, después del renderizado inicial.
    useEffect(() => {
        const temaGuardado = localStorage.getItem('tema') as Tema | null;
        if (temaGuardado) {
            setTema(temaGuardado);
        }
    }, []); // El array vacío asegura que solo se ejecute al montar el componente

    const cambiarTema = useCallback(() => {
        setTema(temaAnterior => {
            const nuevoTema = temaAnterior === 'claro' ? 'oscuro' : 'claro';
            localStorage.setItem('tema', nuevoTema); // Guardamos la preferencia
            return nuevoTema;
        });
    }, []);
    
    // Este efecto actualiza el atributo en el HTML cada vez que el estado del tema cambia
    useEffect(() => {
        document.documentElement.setAttribute('data-tema', tema);
    }, [tema]);

    return (
        <TemaContext.Provider value={{ tema, cambiarTema }}>
            {children}
        </TemaContext.Provider>
    );
}

export function useTema() {
    const context = useContext(TemaContext);
    if (context === undefined) {
        throw new Error('useTema debe ser usado dentro de un TemaProvider');
    }
    return context;
}