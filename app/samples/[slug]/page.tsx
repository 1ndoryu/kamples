import { Suspense } from 'react';
import { notFound } from 'next/navigation';
import { obtenerSamplePorSlug } from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample';
import type { Metadata } from 'next';
import SkeletonSample from '../loaders/skeletonSample';

type PageProps = {
    params: Promise<{ slug: string }>;
};
export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
    const { slug } = await params;
    const sample = await obtenerSamplePorSlug(slug);

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

export default async function PaginaDeSample({ params }: PageProps) {
    const { slug } = await params;

    return (
        <div>
            <Suspense fallback={<SkeletonSample />}>
                <SampleLoader slug={slug} />
            </Suspense>
        </div>
    );
}