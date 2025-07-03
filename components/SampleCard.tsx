'use client';

import styles from "./SampleCard.module.css";
import { isDebug } from "../lib/debug";
import { useState, useEffect } from "react";
import { apiFetch } from "../lib/api";
import { useInView } from "../hooks/useInView";
import LikeButton from "./LikeButton";
import CommentsSection from "./CommentsSection";
import { useAuthStore } from "../store/authStore";

interface Props {
  sample: any;
  onDeleted?: (id: number) => void;
}

export default function SampleCard({ sample, onDeleted }: Props) {
  const [expandido, setExpandido] = useState(false);
  const [mostrarComentarios, setMostrarComentarios] = useState(false);

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

  // NUEVO: Imagen de portada
  const coverImageId: string | number | null = sample?.content_data?.cover_image_id ?? null;

  // Previsualización (ligera)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  // Original solo en debug
  const [originalUrl, setOriginalUrl] = useState<string | null>(null);
  // URL de la imagen de portada
  const [coverImageUrl, setCoverImageUrl] = useState<string | null>(null);

  // Estado para ocultar la tarjeta al eliminar
  const [eliminado, setEliminado] = useState(false);

  // Usuario actual desde el store
  const currentUser = useAuthStore((s) => s.user);

  const puedeBorrar = currentUser && currentUser.id === sample.user_id;

  const handleDelete = async () => {
    if (!confirm("¿Seguro que deseas borrar este sample? Esta acción es irreversible.")) return;

    try {
      await apiFetch<null>(`/contents/${sample.id}`, { method: "DELETE" });
      // Notificamos al padre si pasaron onDeleted via props, o simplemente ocultamos
      if (typeof onDeleted === "function") {
        onDeleted(sample.id);
      } else {
        setEliminado(true);
      }
    } catch (e: any) {
      alert(e.message || "Error eliminando el sample");
    }
  };

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

  // NUEVO: Efecto para obtener la imagen de portada
  useEffect(() => {
    if (!coverImageId || !inView) return;

    (async () => {
      try {
        const res = await apiFetch<any>(`/media/${coverImageId}`);
        const path = res?.data?.path;
        if (path) setCoverImageUrl(`/api/${path}`);
      } catch (e) {
        if (isDebug) console.error("Error obteniendo cover image", e);
      }
    })();
  }, [coverImageId, inView]);

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

  if (eliminado) return null;

  return (
    <article ref={cardRef} className={`${styles.tarjetaSample} bloque`}>
      <h3>{sample.content_data?.title ?? sample.slug}</h3>
      {coverImageUrl && (
        <img
          src={coverImageUrl}
          alt="Imagen de portada"
          style={{ width: "100%", maxHeight: 300, objectFit: "cover" }}
        />
      )}
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

      {/* Botones de interacción */}
      <div style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 8 }}>
        <LikeButton contentId={sample.id} enabled={inView} />
        <details onToggle={(e) => setMostrarComentarios(e.currentTarget.open)}>
          <summary>Comentarios</summary>
          {mostrarComentarios && inView && (
            <CommentsSection contentId={sample.id} />
          )}
        </details>
        {puedeBorrar && (
          <button
            type="button"
            onClick={handleDelete}
            style={{ background: "transparent", border: "none", color: "red", cursor: "pointer" }}
            title="Eliminar sample"
          >
            🗑️
          </button>
        )}
      </div>

      {renderDebugInfo()}
    </article>
  );
} 