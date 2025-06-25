'use client';

import { createContext, useState, useEffect, useContext, ReactNode, useCallback } from 'react';

type Tema = 'claro' | 'oscuro';

interface TemaContextType {
    tema: Tema;
    cambiarTema: () => void;
}

export const TemaContext = createContext<TemaContextType | undefined>(undefined);

export function TemaProvider({ children }: { children: ReactNode }) {
    const [tema, setTema] = useState<Tema>('oscuro');

    useEffect(() => {
        const temaGuardado = localStorage.getItem('tema') as Tema | null;
        if (temaGuardado) {
            setTema(temaGuardado);
        }
    }, []);     

    const cambiarTema = useCallback(() => {
        setTema(temaAnterior => {
            const nuevoTema = temaAnterior === 'claro' ? 'oscuro' : 'claro';
            localStorage.setItem('tema', nuevoTema); 
            return nuevoTema;
        });
    }, []);
    
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