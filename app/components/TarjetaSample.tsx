// app/components/TarjetaSample.tsx
'use client';

import type { Sample } from '@/types/sample';
import Link from 'next/link';

interface Props {
    sample: Sample;
}

export default function TarjetaSample({ sample }: Props) {
    return (
        <article className="tarjetaSample">
            <Link href={`/samples/${sample.slug}`} className="tarjetaSampleEnlace">
                <h3 className="tarjetaSampleTitulo">{sample.titulo}</h3>
                <p className="tarjetaSampleContenido">{sample.contenido}</p>
            </Link>

            <style jsx>{`
                .tarjetaSample {
                    border: 1px solid var(--color-borde); /* Usamos variable */
                    border-radius: 8px;
                    padding: 16px;
                    transition: background-color 0.2s, border-color 0.2s;
                }
                .tarjetaSample:hover {
                    background-color: var(--color-tarjeta-fondo-hover); /* Usamos variable */
                }
                .tarjetaSampleEnlace {
                    text-decoration: none;
                    color: inherit;
                    display: block;
                }
                .tarjetaSampleTitulo {
                    margin: 0 0 8px 0;
                    font-size: 1.2rem;
                    color: var(--color-texto); /* Usamos variable */
                }
                .tarjetaSampleContenido {
                    margin: 0;
                    font-size: 0.9rem;
                    opacity: 0.8;
                    color: var(--color-texto); /* Usamos variable */
                }
            `}</style>
        </article>
    );
}