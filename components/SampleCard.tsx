'use client';

import styles from "./SampleCard.module.css";
import { isDebug } from "../lib/debug";
import { useState, useEffect } from "react";
import { apiFetch } from "../lib/api";
import { useInView } from "../hooks/useInView";

interface Props {
  sample: any;
}

export default function SampleCard({ sample }: Props) {
  const [expandido, setExpandido] = useState(false);

  // Detectamos visibilidad en viewport
  const [cardRef, inView] = useInView<HTMLElement>({ threshold: 0.25 });

  // Identificadores de los distintos audios
  const lightMediaId: string | number | null = sample?.content_data?.light_media_id ?? null;
  const fallbackMediaId: string | number | null = sample?.content_data?.media_id ?? null;

  // El reproductor principal utiliza la versión ligera, o en su defecto la estándar
  const mediaId: string | number | null = lightMediaId ?? fallbackMediaId;

  // En modo debug mostraremos un reproductor adicional con la versión original
  const originalMediaId: string | number | null =
    sample?.content_data?.original_media_id ?? fallbackMediaId;

  // Previsualización (ligera)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  // Original solo en debug
  const [originalUrl, setOriginalUrl] = useState<string | null>(null);

  // Efecto para obtener la URL de la versión ligera / fallback
  useEffect(() => {
    if (!mediaId || !inView) return;

    (async () => {
      try {
        const res = await apiFetch<any>(`/media/${mediaId}`);
        const path = res?.data?.path;
        if (path) setPreviewUrl(`/api/${path}`);
      } catch (e) {
        if (isDebug) console.error("Error obteniendo media preview", e);
      }
    })();
    // solo una vez por mediaId
  }, [mediaId, inView]);

  // Efecto para obtener la URL del original (solo debug)
  useEffect(() => {
    if (!isDebug || !originalMediaId || !inView) return;

    (async () => {
      try {
        const res = await apiFetch<any>(`/media/${originalMediaId}`);
        const path = res?.data?.path;
        if (path) setOriginalUrl(`/api/${path}`);
      } catch (e) {
        if (isDebug) console.error("Error obteniendo media original", e);
      }
    })();
  }, [originalMediaId, inView]);

  const renderDebugInfo = () => {
    // Si no hay datos o no estamos en modo debug, no renderizar nada
    if (!isDebug || !sample.content_data) return null;

    // Convertir el objeto en pares clave / valor ordenados por clave
    const entradas = Object.entries(sample.content_data).sort((a, b) =>
      a[0].localeCompare(b[0])
    );

    // Mostrar solo las primeras 5 entradas si no está expandido
    const entradasMostradas = expandido ? entradas : entradas.slice(0, 5);

    return (
      <div className={styles.debugInfo}>
        <ul>
          {entradasMostradas.map(([clave, valor]) => (
            <li key={clave}>
              <strong>{clave}:</strong>{" "}
              {Array.isArray(valor) ? valor.join(", ") : String(valor)}
            </li>
          ))}
        </ul>

        {/* Información adicional del media */}
        {isDebug && previewUrl && (
          <>
            <strong>URL:</strong> {previewUrl}
          </>
        )}

        {entradas.length > 5 && (
          <button
            type="button"
            onClick={() => setExpandido(!expandido)}
            className={styles.botonExpandir}
          >
            {expandido ? "Ocultar" : "Expandir"}
          </button>
        )}
      </div>
    );
  };

  return (
    <article ref={cardRef} className={`${styles.tarjetaSample} bloque`}>
      <h3>{sample.content_data?.title ?? sample.slug}</h3>
      <p>Tipo: {sample.type}</p>
      <p>Publicado por usuario #{sample.user_id}</p>
      {/* Reproductor principal (ligero) */}
      {previewUrl && (
        <audio controls src={previewUrl}>
          Tu navegador no soporta la reproducción de audio.
        </audio>
      )}

      {/* Reproductor original visible solo en modo debug */}
      {isDebug && originalUrl && (
        <div style={{ marginTop: 8 }}>
          <p style={{ fontSize: 10, opacity: 0.7, margin: 0 }}>Audio original</p>
          <audio controls src={originalUrl}>
            Tu navegador no soporta la reproducción de audio.
          </audio>
        </div>
      )}

      {renderDebugInfo()}
    </article>
  );
} 