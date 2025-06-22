// app/components/forms/previews/PreviewAudio.tsx
'use client';

import {useEffect, useState} from 'react';

interface Props {
    archivo: File;
    onEliminar: () => void;
    onClick: () => void; // Para reemplazar el archivo
}

export default function PreviewAudio({archivo, onEliminar, onClick}: Props) {
    const [urlPreview, setUrlPreview] = useState<string | null>(null);

    useEffect(() => {
        const url = URL.createObjectURL(archivo);
        setUrlPreview(url);
        return () => URL.revokeObjectURL(url);
    }, [archivo]);

    return (
        <div className="previewItem audio" onClick={onClick}>
            {urlPreview && <audio controls src={urlPreview} onClick={e => e.stopPropagation()} />}
            <p title={archivo.name}>{archivo.name}</p>
            <button
                className="botonEliminar"
                onClick={e => {
                    e.stopPropagation(); // Evita que se dispare el onClick del div principal
                    onEliminar();
                }}>
                &times;
            </button>
            <style jsx>{`
                .previewItem.audio {
                    position: relative;
                    display: flex;
                    flex-direction: column;
                    gap: 8px;
                    padding: 1rem;
                    border: 1px solid var(--borde);
                    border-radius: var(--radius);
                    cursor: pointer;
                    transition: border-color 0.2s;
                }
                .previewItem.audio:hover {
                    border-color: var(--color-primario);
                }
                .previewItem.audio audio {
                    width: 100%;
                    max-width: 200px;
                }
                .previewItem.audio p {
                    margin: 0;
                    font-size: 0.8rem;
                    white-space: nowrap;
                    overflow: hidden;
                    text-overflow: ellipsis;
                    max-width: 200px;
                }
                .botonEliminar {
                    color: #fff;
                    cursor: pointer;
                    opacity: 0.7;
                    background: #00000080;
                    border: none;
                    border-radius: 50%;
                    justify-content: center;
                    align-items: center;
                    width: 20px;
                    height: 20px;
                    font-size: 1.2rem;
                    line-height: 1;
                    /* transition: opacity .2s; */
                    display: flex;
                    position: absolute;
                    top: 4px;
                    right: 4px;
                    padding: 7px;
                    margin: 5px;
                }
                .botonEliminar:hover {
                    opacity: 1;
                }
                .previewItem {
                    border: var(--borde) !important;
                    border-radius: var(--radius);
                    gap: 1rem;
                    padding: 1rem;
                    display: flex;
                    flex-direction: row;
                    flex-wrap: nowrap;
                    align-items: center;
                    justify-content: center;
                    align-content: center;
                    width: 100%;
                    max-height: 300px;
                }
            `}</style>
        </div>
    );
}
