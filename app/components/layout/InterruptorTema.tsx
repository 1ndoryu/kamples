// app/components/layout/InterruptorTema.tsx
'use client';

import { useTema } from '@/context/TemaContext';

export default function InterruptorTema() {
    const { tema, cambiarTema } = useTema();

    return (
        <>
            <button onClick={cambiarTema} className="interruptorTema">
                {tema === 'claro' ? '🌙' : '☀️'}
            </button>
            <style jsx>{`
                .interruptorTema {
                    background: none;
                    border: 1px solid var(--color-borde);
                    color: var(--color-texto);
                    padding: 0.5rem;
                    border-radius: 8px;
                    cursor: pointer;
                    font-size: 1.2rem;
                    line-height: 1;
                    transition: background-color 0.2s, border-color 0.2s;
                }
                .interruptorTema:hover {
                    background-color: var(--color-tarjeta-fondo-hover);
                }
            `}</style>
        </>
    );
}