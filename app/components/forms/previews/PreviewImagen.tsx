// app/components/forms/previews/PreviewImagen.tsx
'use client';

import { useEffect, useState } from 'react';

interface Props {
    archivo: File;
}

export default function PreviewImagen({ archivo }: Props) {
    const [urlPreview, setUrlPreview] = useState<string | null>(null);

    useEffect(() => {
        const url = URL.createObjectURL(archivo);
        setUrlPreview(url);

        return () => URL.revokeObjectURL(url);
    }, [archivo]);

    return (
        <div className="previewItem">
            {urlPreview && <img src={urlPreview} alt={archivo.name} />}
            <style jsx>{`
                .previewItem img {
                    max-width: 100px;
                    max-height: 100px;
                    border-radius: var(--radius);
                }
            `}</style>
        </div>
    );
}