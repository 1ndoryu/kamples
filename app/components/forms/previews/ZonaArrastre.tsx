// app/components/forms/previews/ZonaArrastre.tsx
'use client';

import {useState, useCallback, useMemo} from 'react';
import PreviewImagen from './PreviewImagen';
import PreviewAudio from './PreviewAudio';
import PreviewArchivo from './PreviewArchivo';

interface Props {
    archivos: File[];
    setArchivos: (archivos: File[]) => void;
}

export default function ZonaArrastre({archivos, setArchivos}: Props) {
    const [estaArrastrando, setEstaArrastrando] = useState(false);

    const manejarArrastre = (e: React.DragEvent<HTMLDivElement>, activo: boolean) => {
        e.preventDefault();
        e.stopPropagation();
        setEstaArrastrando(activo);
    };

    const manejarSoltar = useCallback(
        (e: React.DragEvent<HTMLDivElement>) => {
            e.preventDefault();
            e.stopPropagation();
            setEstaArrastrando(false);
            if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
                // Convertir FileList a Array y añadir a los existentes
                const nuevosArchivos = Array.from(e.dataTransfer.files);
                setArchivos([...archivos, ...nuevosArchivos]);
            }
        },
        [archivos, setArchivos]
    );

    const previews = useMemo(() => {
        return archivos.map((archivo, index) => {
            if (archivo.type.startsWith('image/')) {
                return <PreviewImagen key={index} archivo={archivo} />;
            } else if (archivo.type.startsWith('audio/')) {
                return <PreviewAudio key={index} archivo={archivo} />;
            } else {
                return <PreviewArchivo key={index} archivo={archivo} />;
            }
        });
    }, [archivos]);

    const tieneArchivos = archivos.length > 0;

    return (
        <div onDragEnter={e => manejarArrastre(e, true)} onDragOver={e => manejarArrastre(e, true)} onDragLeave={e => manejarArrastre(e, false)} onDrop={manejarSoltar} className={`zonaArrastre ${estaArrastrando ? 'activo' : ''} ${tieneArchivos ? 'conArchivos' : ''}`} id="ppp3">
            {tieneArchivos ? (
                <div className="previewsContenedor NGEESM">{previews}</div>
            ) : (
                <div className="mensajeArrastre">
                    <p>Arrastra y suelta tus archivos aquí</p>
                </div>
            )}
            <style jsx>{`
                .zonaArrastre {
                    border: var(--borde);
                    border-radius: var(--radius);
                    padding: 1rem;
                    text-align: center;
                    transition: background-color 0.2s, border-color 0.2s;
                    min-height: 100px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                }
                .zonaArrastre.activo {
                    background-color: var(--color-tarjeta-fondo-hover);
                    border-color: var(--color-primario);
                }
                .zonaArrastre.conArchivos {
                    border-style: solid;
                }
                .previewsContenedor {
                    display: grid;
                    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
                    gap: 1rem;
                    width: 100%;
                }
            `}</style>
        </div>
    );
}
