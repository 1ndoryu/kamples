// app/samples/[slug]/page.tsx
import { Suspense } from 'react';
import { notFound } from 'next/navigation';
import { obtenerSamplePorSlug } from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample';
import type { Metadata } from 'next';

// -----------------------------------------------------------------
// 1. REMOVE the custom 'PageProps' type. It is not needed.
// -----------------------------------------------------------------
// type PageProps = {
//     params: { slug: string };
// };

// 2. Type the props directly in the function signatures.
// This allows TypeScript to correctly infer the types from Next.js.
export async function generateMetadata(
    { params }: { params: { slug: string } }
): Promise<Metadata> {
    const sample = await obtenerSamplePorSlug(params.slug);

    if (!sample) {
        return { title: 'Sample no encontrado' };
    }
    return {
        title: `${sample.titulo} - Kamples`,
        description: sample.subtitulo || sample.contenido?.substring(0, 150) || '',
    };
}

async function SampleLoader({ slug }: { slug: string }) {
    const sample = await obtenerSamplePorSlug(slug);
    if (!sample) {
        notFound();
    }
    return <DetalleSample sample={sample} />;
}

// 3. Apply the same inline typing to your default page component.
export default function PaginaDeSample(
    { params }: { params: { slug: string } }
) {
    return (
        <div>
            <Suspense fallback={<div className="cargandoContenido">Cargando sample...</div>}>
                <SampleLoader slug={params.slug} />
            </Suspense>
        </div>
    );
}