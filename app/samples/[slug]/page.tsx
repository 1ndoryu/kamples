// app/samples/[slug]/page.tsx
import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { obtenerSamplePorSlug } from '@/services/swordApi';
import Tabs, { type Pestaña } from '@/components/ui/Tabs'; // <-- 1. Importar Tabs y el tipo Pestaña

type Props = {
  params: { slug: string };
};

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const sample = await obtenerSamplePorSlug(params.slug);
  if (!sample) {
    return { title: 'Sample no encontrado' };
  }
  return {
    title: `${sample.titulo} - Kamples`,
    description: sample.subtitulo || sample.contenido.substring(0, 150),
  };
}

export default async function PaginaDeSample({ params }: Props) {
  const { slug } = params;
  const sample = await obtenerSamplePorSlug(slug);

  if (!sample) {
    notFound();
  }

  // --- 2. Preparar el contenido para las pestañas ---
  const pestañasDelSample: Pestaña[] = [
    {
      etiqueta: 'Detalles',
      contenido: (
        <div className="metadataSample">
          <ul>
            <li>ID Autor: {sample.idautor}</li>
            <li>Publicado: {new Date(sample.created_at).toLocaleDateString('es-ES')}</li>
            {/* Aquí se pueden mostrar los metadatos como BPM, Tonalidad, etc. */}
            {Object.entries(sample.metadata).map(([clave, valor]) => (
              <li key={clave}>
                <span className="claveMetadata">{clave.replace(/_/g, ' ')}:</span> {String(valor)}
              </li>
            ))}
          </ul>
        </div>
      ),
    },
    {
      etiqueta: 'Comentarios',
      contenido: (
        <div>
          <p>[Aquí irá el futuro componente para listar y añadir comentarios.]</p>
        </div>
      ),
    },
    {
      etiqueta: 'Descargas',
      contenido: (
        <div>
          <p>[Aquí irá la información de descargas y el botón principal.]</p>
        </div>
      ),
    },
  ];

  return (
    <>
      <article className="vistaSample">
        <h1>{sample.titulo}</h1>
        {sample.subtitulo && <h2>{sample.subtitulo}</h2>}

        <div className="reproductorAudio">
          <p>[Aquí irá el reproductor de audio del sample]</p>
        </div>

        {/* --- 3. Renderizar el componente de Tabs --- */}
        <Tabs pestañas={pestañasDelSample} />

      </article>

      {/* CSS mínimo para mejorar la presentación */}
      <style jsx>{`
        .vistaSample {
          max-width: 800px;
          margin: 2rem auto;
          padding: 0 2rem;
        }
        .reproductorAudio {
          background-color: var(--color-fondo-secundario);
          border: 1px solid var(--color-borde);
          padding: 2rem;
          text-align: center;
          border-radius: 8px;
          margin-bottom: 2rem;
        }
        /* Estilos movidos aquí desde el componente Tabs para mantenerlo limpio */
        .metadataSample ul {
          list-style: none;
          padding: 0;
        }
        .metadataSample li {
          display: flex;
          justify-content: space-between;
          padding: 0.75rem 0;
          border-bottom: 1px solid var(--color-borde);
        }
        .metadataSample li:last-child {
          border-bottom: none;
        }
        .claveMetadata {
          font-weight: 600;
          text-transform: capitalize;
        }
      `}</style>
    </>
  );
}