// app/samples/[slug]/page.tsx
import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { obtenerSamplePorSlug } from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample';

// CORRECCIÓN: Se elimina el tipo 'Props' y se define el tipo de los parámetros de forma "inline".
export async function generateMetadata(
    { params }: { params: { slug: string } }
): Promise<Metadata> {
    const sample = await obtenerSamplePorSlug(params.slug);
    if (!sample) {
        return { title: 'Sample no encontrado' };
    }
    return {
        title: `${sample.titulo} - Kamples`,
        description: sample.subtitulo || sample.contenido?.substring(0, 150),
    };
}

// CORRECCIÓN: Se aplica la misma lógica de tipado inline para el componente de la página.
export default async function PaginaDeSample({ params }: { params: { slug: string } }) {
    const { slug } = params;
    const sample = await obtenerSamplePorSlug(slug);

    if (!sample) {
        notFound();
    }

    // El componente de cliente se encarga de la presentación.
    return <DetalleSample sample={sample} />;
}