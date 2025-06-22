// app/components/layout/InterruptorTema.tsx
'use client';

import {useTema} from '@/context/TemaContext';

export default function InterruptorTema() {
    const {tema, cambiarTema} = useTema();

    return (
        <>
            <button onClick={cambiarTema} className="interruptorTema">
                {tema === 'claro' ? '🌙' : '☀️'}
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
