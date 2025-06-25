'use client';

import type { Sample } from '@/types/sample';
import Tabs, { type Pestaña } from '@/components/ui/Tabs';
import Comentarios from './Comentarios';
import DetalleItem from './ui/DetalleItem';
import ClockIcon from './icons/ClockIcon';
import KeyIcon from './icons/KeyIcon';
import TagIcon from './icons/TagIcon';

interface Props {
    sample: Sample;
}

const getIconForMetadato = (clave: string) => {
    switch (clave.toLowerCase()) {
        case 'bpm':
            return <ClockIcon className="w-6 h-6 text-purple-400" />;
        case 'tonalidad':
            return <KeyIcon className="w-6 h-6 text-teal-400" />;
        default:
            return <TagIcon className="w-6 h-6 text-sky-400" />;
    }
};

export default function DetalleSample({ sample }: Props) {
    const detallesPrincipales = {
        Autor: sample.idautor,
        Publicado: new Date(sample.created_at).toLocaleDateString('es-ES'),
    };

    const { ...caracteristicasAudio } = sample.metadata;

    const pestañasDelSample: Pestaña[] = [
        {
            etiqueta: 'Detalles',
            contenido: (
                <div className="p-4">
                    <h3 className="text-lg font-semibold mb-4 text-gray-800 dark:text-gray-200">Detalles Principales</h3>
                    <div className="detalles-grid-container mb-6">
                        {Object.entries(detallesPrincipales).map(([clave, valor]) => (
                            <DetalleItem
                                key={clave}
                                icon={<TagIcon className="w-6 h-6 text-gray-400" />} // Icono genérico
                                label={clave}
                                value={String(valor)}
                            />
                        ))}
                    </div>

                    <h3 className="text-lg font-semibold mb-4 text-gray-800 dark:text-gray-200">Características de Audio</h3>
                    <div className="detalles-grid-container">
                        {Object.entries(caracteristicasAudio).map(([clave, valor]) => (
                            <DetalleItem
                                key={clave}
                                icon={getIconForMetadato(clave)}
                                label={clave.replace(/_/g, ' ')}
                                value={String(valor)}
                            />
                        ))}
                    </div>
                </div>
            )
        },
        {
            etiqueta: 'Comentarios',
            contenido: <Comentarios sampleId={sample.id} />
        },
        {
            etiqueta: 'Descargas',
            contenido: (
                <div className="p-4">
                    <p>[Aquí irá la información de descargas y el botón principal.]</p>
                </div>
            )
        }
    ];

    return (
        <>
            <article className="vistaSample">
                <h1>{sample.titulo}</h1>
                {sample.subtitulo && <h2>{sample.subtitulo}</h2>}

                <div className="reproductorAudio">
                    <p>[Aquí irá el reproductor de audio del sample]</p>
                </div>

                <Tabs pestañas={pestañasDelSample} />
            </article>

            <style jsx>{`
                .vistaSample {
                    max-width: 800px;
                    margin: 2rem auto;
                    padding: 0 2rem;
                }
                .reproductorAudio {
                    background-color: var(--color-fondo-secundario);
                    border: 1px solid var(--color-borde);
                    padding: 2rem;
                    text-align: center;
                    border-radius: 8px;
                    margin-bottom: 2rem;
                }
                .detalles-grid-container {
                    display: grid;
                    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
                    gap: 1rem;
                    padding: 1.5rem;
                    width: 100%;
                }
                
                :global(.detalle-item-container) {
                    /* Fondo y borde eliminados para un look más limpio */
                    border-radius: 8px; /* Mantenemos el radio para el efecto hover */
                    padding: 0.75rem;
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                    transition: background-color 0.2s ease-in-out;
                }

                :global(.detalle-item-container:hover) {
                    background-color: var(--color-fondo-secundario); /* Fondo sutil en hover */
                }

                :global(.icon-container) {
                    flex-shrink: 0;
                }

                :global(.icon-container svg) {
                    width: 1.5rem; /* 24px */
                    height: 1.5rem; /* 24px */
                    color: #888;
                }

                :global(.text-container) {
                    display: flex;
                    flex-direction: column;
                }

                :global(.label) {
                    font-size: 0.8rem; /* Tamaño de letra reducido */
                    color: #aaa;
                    text-transform: capitalize;
                }

                :global(.value) {
                    font-size: 1rem; /* Tamaño de letra reducido */
                    font-weight: 500;
                    color: #fff;
                }
            `}</style>
        </>
    );
}

