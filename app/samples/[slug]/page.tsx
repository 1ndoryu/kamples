// app/samples/[slug]/page.tsx
import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { obtenerSamplePorSlug } from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample';
import type { Sample } from '@/types/sample';

// CORRECCIÓN: Definimos una interfaz completa que coincide con la de Next.js,
// incluyendo `params` y `searchParams` para máxima compatibilidad.
interface PageProps {
  params: { slug: string };
  searchParams?: { [key: string]: string | string[] | undefined };
}

export async function generateMetadata(
    { params }: PageProps
): Promise<Metadata> {
    const sample: Sample | null = await obtenerSamplePorSlug(params.slug);

    if (!sample) {
        return { title: 'Sample no encontrado' };
    }
    return {
        title: `${sample.titulo} - Kamples`,
        description: sample.subtitulo || sample.contenido?.substring(0, 150) || '',
    };
}

export default async function PaginaDeSample({ params }: PageProps) {
    const { slug } = params;
    const sample: Sample | null = await obtenerSamplePorSlug(slug);

    if (!sample) {
        notFound();
    }

    // A partir de aquí, TypeScript sabe que 'sample' no es null.
    return <DetalleSample sample={sample} />;
}