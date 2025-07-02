import { useState, useEffect } from "react";
import { apiFetch } from "../lib/api";
import { useAuthStore } from "../store/authStore";
import styles from "./LikeButton.module.css";

interface Props {
  contentId: number;
  enabled?: boolean; // Si es false, no se hace la petición aún
}

export default function LikeButton({ contentId, enabled = true }: Props) {
  const { token } = useAuthStore();
  const [cuentaLikes, setCuentaLikes] = useState<number>(0);
  const [cargando, setCargando] = useState<boolean>(false);

  // Cargar likes sólo cuando enabled === true
  useEffect(() => {
    if (!enabled) return;

    async function cargarLikes() {
      try {
        const res = await apiFetch<{ like_count: number }>(
          `/contents/${contentId}/likes`
        );
        setCuentaLikes(res.data.like_count);
      } catch (_) {
        // Silenciamos errores
      }
    }
    cargarLikes();
  }, [contentId, enabled]);

  const alternarLike = async () => {
    if (!token || cargando) return; // Requiere autenticación
    setCargando(true);
    try {
      const res = await apiFetch<{ like_count: number }>(
        `/contents/${contentId}/like`,
        { method: "POST" }
      );
      setCuentaLikes(res.data.like_count);
    } catch (_) {
      // Silenciar
    } finally {
      setCargando(false);
    }
  };

  return (
    <button
      className={styles.botonLike}
      onClick={alternarLike}
      disabled={!token || cargando}
      title={token ? "Dar me gusta" : "Inicia sesión para dar me gusta"}
    >
      ❤️ {cuentaLikes}
    </button>
  );
}