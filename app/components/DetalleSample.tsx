'use client';

import type {Sample} from '@/types/sample';
import Tabs, {type Pestaña} from '@/components/ui/Tabs';

interface Props {
    sample: Sample;
}

export default function DetalleSample({sample}: Props) {
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
