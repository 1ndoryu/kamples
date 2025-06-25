'use client';

import {useEffect, useState} from 'react';
import Image from 'next/image';

interface Props {
    archivo: File;
    onEliminar: () => void;
    onClick: () => void;
}

export default function PreviewImagen({archivo, onEliminar, onClick}: Props) {
    const [urlPreview, setUrlPreview] = useState<string | null>(null);

    useEffect(() => {
        const url = URL.createObjectURL(archivo);
        setUrlPreview(url);

        return () => URL.revokeObjectURL(url);
    }, [archivo]);

    return (
        <div className="previewItem imagen" onClick={onClick}>
            {urlPreview && <Image src={urlPreview} alt={archivo.name} fill style={{ objectFit: 'cover' }}/>}
            <button
                className="botonEliminar"
                onClick={e => {
                    e.stopPropagation();
                    onEliminar();
                }}>
                &times;
            </button>
            <style jsx>{`
                .previewItem.imagen {
                    position: relative;
                    cursor: pointer;
                    border: 1px solid var(--borde);
                    border-radius: var(--radius);
                    overflow: hidden; /* Para que el radio se aplique a la imagen */
                    height: 300px;
                }
                .previewItem.imagen:hover::after {
                    content: 'Reemplazar';
                    position: absolute;
                    inset: 0;
                    background: rgba(0, 0, 0, 0.5);
                    color: white;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    font-size: 0.9rem;
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
                    z-index: 1;
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