// Regla 5: SEO es lo más importante.
// Usaremos las funciones de Next.js para generar metadatos dinámicos.
import type {Metadata, ResolvingMetadata} from 'next';
import {notFound} from 'next/navigation';
import {obtenerSamplePorSlug} from '@/services/swordApi';

// Props que recibe la página y la función de metadatos.
// Next.js nos las pasa automáticamente.
type Props = {
    params: {slug: string};
};

// --- METADATOS DINÁMICOS PARA SEO (Regla #5) ---
// Esta función se ejecuta en el servidor ANTES de renderizar la página.
// Genera el <title> y <meta name="description"> específicos para este sample.
export async function generateMetadata({params}: Props): Promise<Metadata> {
    const sample = await obtenerSamplePorSlug(params.slug);

    if (!sample) {
        // Si no encontramos el sample, no generamos metadatos específicos.
        return {
            title: 'Sample no encontrado'
        };
    }

    return {
        title: `${sample.titulo} - Kamples`,
        description: sample.subtitulo || sample.contenido.substring(0, 150) // Usa el subtítulo o un extracto del contenido
        // En el futuro, podríamos añadir más metadatos para redes sociales (Open Graph)
        // openGraph: {
        //   title: sample.titulo,
        //   description: sample.contenido,
        //   images: [ ... ],
        // }
    };
}

// --- PÁGINA DEL SAMPLE (Componente de Servidor) ---
export default async function PaginaDeSample({params}: Props) {
    const {slug} = params;
    const sample = await obtenerSamplePorSlug(slug);

    // Si la API devuelve null (no encontró el sample), mostramos la página 404 de Next.js.
    // Esto es importante para el SEO y la experiencia de usuario.
    if (!sample) {
        notFound();
    }

    return (
        <article className="vistaSample">
            {/* Aquí iría la información detallada del sample */}
            <h1>{sample.titulo}</h1>
            {sample.subtitulo && <h2>{sample.subtitulo}</h2>}

            <div className="reproductorAudio">
                {/* Futuro componente de reproductor de audio */}
                <p>[Aquí irá el reproductor de audio del sample]</p>
            </div>

            <div className="contenidoSample">
                <p>{sample.contenido}</p>
            </div>

            <div className="metadataSample">
                <h3>Detalles</h3>
                <ul>
                    <li>ID Autor: {sample.idautor}</li>
                    <li>Publicado: {new Date(sample.created_at).toLocaleDateString('es-ES')}</li>
                    {/* Aquí se pueden mostrar los metadatos como BPM, Tonalidad, etc. */}
                    {Object.entries(sample.metadata).map(([clave, valor]) => (
                        <li key={clave}>
                            {clave}: {String(valor)}
                        </li>
                    ))}
                </ul>
            </div>
        </article>
    );
}
