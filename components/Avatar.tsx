import { useEffect, useState } from "react";
import styles from "./Avatar.module.css";
import { apiFetch, API_URL } from "../lib/api";

interface AvatarProps {
  /** ID del usuario cuyo avatar queremos mostrar. Opcional si se pasa avatarUrl */
  userId?: number | string;
  /** Ruta relativa (o absoluta) del avatar. Opcional si se pasa userId */
  avatarUrl?: string | null;
  /** Tamaño en píxeles del avatar (ancho y alto) */
  size?: number;
}

/**
 * Pequeño componente que muestra la foto de perfil del usuario.
 * Si recibe avatarUrl la utiliza directamente; si sólo recibe userId hace una
 * petición a la API para obtener profile_data.avatar_url de ese usuario.
 */
export default function Avatar({ userId, avatarUrl: propAvatarUrl, size = 32 }: AvatarProps) {
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);

  useEffect(() => {
    // Si ya viene la URL del avatar, la usamos directamente
    if (propAvatarUrl) {
      setAvatarUrl(formatUrl(propAvatarUrl));
      return;
    }

    // Si no tenemos userId no podemos continuar
    if (!userId) return;

    (async () => {
      try {
        const res = await apiFetch<any>(`/users/${userId}`);
        const fetchedUrl = res?.data?.profile_data?.avatar_url ?? null;
        if (fetchedUrl) {
          setAvatarUrl(formatUrl(fetchedUrl));
        } else {
          // Sin avatar -> placeholder
          setAvatarUrl(null);
        }
      } catch (e: any) {
        // Si es 404, simplemente ignoramos para evitar ruido.
        if (process.env.NODE_ENV === "development") {
          console.debug("Avatar no encontrado para user", userId);
        }
        setAvatarUrl(null);
      }
    })();
  }, [userId, propAvatarUrl]);

  if (!avatarUrl) {
    // Placeholder sencillo: círculo gris
    return (
      <div
        className={styles.avatar}
        style={{ width: size, height: size, background: "#444" }}
      />
    );
  }
  return (
    <img
      src={avatarUrl}
      alt="avatar"
      className={styles.avatar}
      style={{ width: size, height: size }}
    />
  );
}

function formatUrl(path: string) {
  // Si la ruta ya es absoluta (http/https) la devolvemos tal cual
  if (/^https?:\/\//i.test(path)) return path;
  // Si empieza por / asume que es absoluta dentro del mismo dominio
  if (path.startsWith("/")) return path;
  // Si no, añadimos el API_URL delante para construir la URL completa
  return `${API_URL}/${path}`;
} 