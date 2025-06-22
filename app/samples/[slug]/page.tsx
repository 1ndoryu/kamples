// app/samples/[slug]/page.tsx
import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { obtenerSamplePorSlug } from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample'; // <-- 1. Importar el nuevo componente cliente

type Props = {
    params: { slug: string };
};

// La función generateMetadata se mantiene igual, se ejecuta en el servidor.
export async function generateMetadata({ params }: Props): Promise<Metadata> {
    const sample = await obtenerSamplePorSlug(params.slug);
    if (!sample) {
        return { title: 'Sample no encontrado' };
    }
    return {
        title: `${sample.titulo} - Kamples`,
        description: sample.subtitulo || sample.contenido?.substring(0, 150),
    };
}

// La página sigue siendo un Server Component para el SEO y la obtención de datos.
export default async function PaginaDeSample({ params }: Props) {
    const { slug } = params;
    const sample = await obtenerSamplePorSlug(slug);

    if (!sample) {
        notFound();
    }

    // 2. Renderizamos el componente cliente, pasándole los datos.
    // Toda la lógica de presentación (incluido styled-jsx) está ahora en DetalleSample.
    return <DetalleSample sample={sample} />;
}