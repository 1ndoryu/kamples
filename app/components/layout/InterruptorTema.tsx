// app/components/layout/InterruptorTema.tsx
'use client';

// import {useTema} from '@/context/TemaContext'; // Eliminado
import { useAppStore } from '@/store/useAppStore'; // Importar el store de Zustand

export default function InterruptorTema() {
    // const {tema, cambiarTema} = useTema(); // Eliminado
    const tema = useAppStore((state) => state.theme);
    const toggleTheme = useAppStore((state) => state.toggleTheme);
    // O usar el selector:
    // import { useCurrentTheme, useThemeActions } from '@/store/useAppStore';
    // const tema = useCurrentTheme();
    // const { toggleTheme } = useThemeActions();


    return (
        <>
            <button onClick={toggleTheme} className="interruptorTema"> {/* Usar toggleTheme del store */}
                {tema === 'claro' ? '🌙' : '☀️'} {/* La lógica del ícono sigue siendo la misma */}
            </button>
            <style jsx>{`
                .interruptorTema {
                    color: var(--color-texto);
                    cursor: pointer;
                    border-radius: var(--radius);
                    background: 0 0;
                    padding: 0.3rem;
                    font-size: 1rem;
                    transition: background-color 0.2s, border-color 0.2s;
                }
                .interruptorTema:hover {
                    background-color: var(--color-tarjeta-fondo-hover);
                }
            `}</style>
        </>
    );
}
