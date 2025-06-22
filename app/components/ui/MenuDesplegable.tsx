// app/components/ui/MenuDesplegable.tsx
'use client';

import {useState, useRef, useEffect, createContext, useContext, type ReactNode} from 'react';

// --- Tipos y Contexto ---
interface MenuContextType {
    abierto: boolean;
    setAbierto: (abierto: boolean) => void;
}

const MenuContext = createContext<MenuContextType | null>(null);

const useMenu = () => {
    const context = useContext(MenuContext);
    if (!context) throw new Error('useMenu debe usarse dentro de un ContenedorMenu');
    return context;
};

// --- Componentes ---

/** Contenedor principal que maneja el estado del menú */
export function ContenedorMenu({children}: {children: ReactNode}) {
    const [abierto, setAbierto] = useState(false);
    const menuRef = useRef<HTMLDivElement>(null);

    // Cierra el menú si se hace clic fuera de él
    useEffect(() => {
        const manejarClickFuera = (evento: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(evento.target as Node)) {
                setAbierto(false);
            }
        };
        document.addEventListener('mousedown', manejarClickFuera);
        return () => document.removeEventListener('mousedown', manejarClickFuera);
    }, []);

    return (
        <MenuContext.Provider value={{abierto, setAbierto}}>
            <div ref={menuRef} className="contenedorMenuPrincipal">
                {children}
            </div>
        </MenuContext.Provider>
    );
}

/** Botón que abre y cierra el menú desplegable */
export function BotonMenu({children, className}: {children: ReactNode; className?: string}) {
    const {abierto, setAbierto} = useMenu();
    return (
        <button type="button" className={`botonMenu ${className || ''}`} aria-haspopup="true" aria-expanded={abierto} onClick={() => setAbierto(!abierto)}>
            {children}
        </button>
    );
}

/** La lista de opciones que aparece y desaparece */
export function ListaMenu({children, ancho = 200}: {children: ReactNode; ancho?: number}) {
    const {abierto} = useMenu();
    if (!abierto) return null;

    return (
        <div className="menuDesplegable" role="menu" style={{width: `${ancho}px`}}>
            <ul>{children}</ul>
        </div>
    );
}

/** Cada uno de los ítems clickeables dentro del menú */
export function ItemMenu({children, onClick, tipo = 'normal'}: {children: ReactNode; onClick: () => void; tipo?: 'normal' | 'peligro'}) {
    const {setAbierto} = useMenu();
    const manejarClick = () => {
        onClick();
        setAbierto(false); // Cierra el menú al hacer clic
    };

    return (
        <li role="menuitem" className={`itemMenu ${tipo}`} onClick={manejarClick}>
            {children}
        </li>
    );
}
