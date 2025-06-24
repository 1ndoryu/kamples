// app/components/DetalleSample.tsx
'use client';

import { useEffect } from 'react';
import type {Sample} from '@/types/sample';
import Tabs, {type Pestaña} from '@/components/ui/Tabs';
import { useAppStore } from '@/store/useAppStore';

interface Props {
    sample: Sample; // Recibe el sample cargado desde el servidor
}

export default function DetalleSample({sample: sampleProp}: Props) { // Renombrado a sampleProp para claridad
    const setSelectedSample = useAppStore((state) => state.setSelectedSample);

    useEffect(() => {
        // Actualizar el selectedSample en el store cuando el componente se monta o el sampleProp cambia.
        // Esto hace que el sample actual esté disponible globalmente si es necesario.
        if (sampleProp) {
            setSelectedSample(sampleProp);
        }
        // Limpiar el selectedSample cuando el componente se desmonte podría ser una opción,
        // dependiendo del comportamiento deseado.
        // return () => setSelectedSample(null);
    }, [sampleProp, setSelectedSample]);

    // El resto del componente usa sampleProp para el renderizado inicial rápido.
    // Si se quisiera que siempre muestre el estado del store (selectedSample),
    // se podría hacer, pero entonces se perdería el beneficio del SSR para este componente.
    const sample = sampleProp; // Usamos el sample de los props para el renderizado

    // Preparar el contenido para las pestañas
    const pestañasDelSample: Pestaña[] = [
        {
            etiqueta: 'Detalles',
            contenido: (
                <div className="metadataSample">
                    <ul>
                        <li>ID Autor: {sample.idautor}</li>
                        <li>Publicado: {new Date(sample.created_at).toLocaleDateString('es-ES')}</li>
                        {/* Aquí se pueden mostrar los metadatos como BPM, Tonalidad, etc. */}
                        {Object.entries(sample.metadata).map(([clave, valor]) => (
                            <li key={clave}>
                                <span className="claveMetadata">{clave.replace(/_/g, ' ')}:</span> {String(valor)}
                            </li>
                        ))}
                    </ul>
                </div>
            )
        },
        {
            etiqueta: 'Comentarios',
            contenido: (
                <div>
                    <p>[Aquí irá el futuro componente para listar y añadir comentarios.]</p>
                </div>
            )
        },
        {
            etiqueta: 'Descargas',
            contenido: (
                <div>
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

                {/* Renderizamos el componente de Tabs que ya es un client component */}
                <Tabs pestañas={pestañasDelSample} />
            </article>

            {/* El styled-jsx ahora vive de forma segura dentro de un Client Component */}
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
                .metadataSample ul {
                    list-style: none;
                    padding: 0;
                }
                .metadataSample li {
                    display: flex;
                    justify-content: space-between;
                    padding: 0.75rem 0;
                    border-bottom: 1px solid var(--color-borde);
                }
                .metadataSample li:last-child {
                    border-bottom: none;
                }
                .claveMetadata {
                    font-weight: 600;
                    text-transform: capitalize;
                }
            `}</style>
        </>
    );
}
