// app/samples/[slug]/page.tsx
import { Suspense } from 'react';
import { notFound } from 'next/navigation';
import { obtenerSamplePorSlug } from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample'; // Client component que mostrará el detalle
import type { Metadata } from 'next';

// Tipado para los parámetros de la página
type PageProps = {
    params: { slug: string }; // El slug ya viene resuelto por Next.js
};

// generateMetadata para SEO
export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
    const { slug } = params; // Acceso directo al slug
    const sample = await obtenerSamplePorSlug(slug);

    if (!sample) {
        return { title: 'Sample no encontrado' };
    }
    return {
        title: `${sample.titulo} - Kamples`,
        description: sample.subtitulo || sample.contenido?.substring(0, 150) || '',
    };
}

// El componente de página sigue siendo un Server Component
export default async function PaginaDeSample({ params }: PageProps) {
    const { slug } = params; // Acceso directo al slug
    const sample = await obtenerSamplePorSlug(slug);

    // Si el sample no se encuentra, mostrar notFound (página 404)
    if (!sample) {
        notFound();
    }

    // Pasamos el sample cargado en el servidor al componente cliente DetalleSample
    // Suspense podría usarse si DetalleSample tuviera alguna carga de datos adicional o componentes lazy-loaded.
    // Por ahora, si DetalleSample es síncrono con los datos del sample, Suspense es menos crítico aquí.
    return (
        <div>
            <Suspense fallback={<div className="cargandoContenido">Cargando sample...</div>}>
                <DetalleSample sample={sample} />
            </Suspense>
        </div>
    );
}