// app/components/forms/previews/ZonaArrastre.tsx
'use client';

import PreviewImagen from './PreviewImagen';
import PreviewAudio from './PreviewAudio';

interface Props {
    archivoAudio: File | null;
    archivoImagen: File | null;
    onAdjuntarAudio: () => void;
    onAdjuntarImagen: () => void;
    onEliminarAudio: () => void;
    onEliminarImagen: () => void;
}

export default function ZonaArrastre({archivoAudio, archivoImagen, onAdjuntarAudio, onAdjuntarImagen, onEliminarAudio, onEliminarImagen}: Props) {
    return (
        // AJUSTE: Este componente ahora solo es el contenedor de previews
        <div className="previewsContenedor">
            {archivoAudio && <PreviewAudio archivo={archivoAudio} onEliminar={onEliminarAudio} onClick={onAdjuntarAudio} />}
            {archivoImagen && <PreviewImagen archivo={archivoImagen} onEliminar={onEliminarImagen} onClick={onAdjuntarImagen} />}
            <style jsx>{`
                .previewsContenedor {
                    flex-wrap: nowrap;
                    justify-content: space-between;
                    gap: 1rem;
                    width: 100%;
                    display: flex;
                    flex-direction: row;
                }
            `}</style>
        </div>
    );
}
