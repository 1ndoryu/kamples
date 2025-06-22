// app/components/forms/previews/PreviewAudio.tsx
'use client';

import {useEffect, useState} from 'react';

interface Props {
    archivo: File;
}

export default function PreviewAudio({archivo}: Props) {
    const [urlPreview, setUrlPreview] = useState<string | null>(null);

    useEffect(() => {
        const url = URL.createObjectURL(archivo);
        setUrlPreview(url);
        return () => URL.revokeObjectURL(url);
    }, [archivo]);

    return (
        <div className="previewItem audio">
            {urlPreview && <audio controls src={urlPreview} />}
            <p>{archivo.name}</p>
            <style jsx>{`
                .previewItem.audio {
                    display: flex;
                    flex-direction: column;
                    gap: 8px;
                    padding: 1rem;
                    border: 1px solid var(--borde);
                    border-radius: var(--radius);
                }
                .previewItem.audio audio {
                    width: 100%;
                }
                .previewItem.audio p {
                    margin: 0;
                    font-size: 0.8rem;
                    white-space: nowrap;
                    overflow: hidden;
                    text-overflow: ellipsis;
                }
            `}</style>
        </div>
    );
}
