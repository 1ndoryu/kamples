// app/components/forms/previews/PreviewArchivo.tsx
'use client';

interface Props {
    archivo: File;
}

export default function PreviewArchivo({archivo}: Props) {
    return (
        <div className="previewItem archivo">
            {/* SVG Placeholder */}
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                <path d="M14 2H6C4.9 2 4.01 2.9 4.01 4L4 20C4 21.1 4.89 22 5.99 22H18C19.1 22 20 21.1 20 20V8L14 2ZM18 20H6V4H13V9H18V20Z" fill="currentColor" />
            </svg>
            <p>{archivo.name}</p>
            <style jsx>{`
                .previewItem.archivo {
                    display: flex;
                    align-items: center;
                    gap: 8px;
                    padding: 1rem;
                    border: 1px solid var(--borde);
                    border-radius: var(--radius);
                }
                .previewItem.archivo p {
                    margin: 0;
                    font-size: 0.8rem;
                }
            `}</style>
        </div>
    );
}
