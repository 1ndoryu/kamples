'use client'; // <-- SOLUCIÓN: Este componente maneja la visualización y necesita ser un Client Component.

import type {Sample} from '@/types/sample';
import TarjetaSample from '@/components/TarjetaSample';

interface Props {
    samples: Sample[];
}

export default function ListaSamples({samples}: Props) {
    return (
        <>
            {samples.length > 0 ? (
                <div className="rejillaSamples">
                    {samples.map(sample => (
                        <TarjetaSample key={sample.id} sample={sample} />
                    ))}
                </div>
            ) : (
                <p>No se encontraron samples en este momento. Intenta de nuevo más tarde.</p>
            )}

            {/* CSS que antes estaba en la página principal, ahora está encapsulado aquí */}
            <style jsx>{`
                .rejillaSamples {
                    display: grid;
                    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
                    gap: 1.5rem;
                    margin-top: 2rem;
                }
            `}</style>
        </>
    );
}
