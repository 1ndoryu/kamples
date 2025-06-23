// app/samples/[slug]/page.tsx
import { Suspense } from 'react';
import { notFound } from 'next/navigation';
import { obtenerSamplePorSlug } from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample';
import type { Metadata } from 'next';

// NO debe haber ninguna definición o importación de 'PageProps' aquí.

export async function generateMetadata(
    { params }: { params: { slug: string } } // <-- Fíjate en la sintaxis
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

export default function PaginaDeSample(
    { params }: { params: { slug:string } } // <-- Fíjate en la sintaxis
) {
    return (
        <div>
            <Suspense fallback={<div className="cargandoContenido">Cargando sample...</div>}>
                <SampleLoader slug={params.slug} />
            </Suspense>
        </div>
    );
}