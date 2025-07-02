import { useState, useEffect, FormEvent } from "react";
import { apiFetch } from "../lib/api";
import { useAuthStore } from "../store/authStore";
import styles from "./CommentsSection.module.css";

interface Props {
  contentId: number;
}

interface Comment {
  id: number;
  user_id: number;
  body: string;
  user?: {
    id: number;
    username: string;
  };
}

export default function CommentsSection({ contentId }: Props) {
  const { token } = useAuthStore();
  const [comentarios, setComentarios] = useState<Comment[]>([]);
  const [cargando, setCargando] = useState<boolean>(false);
  const [nuevoComentario, setNuevoComentario] = useState<string>("");

  const cargarComentarios = async () => {
    setCargando(true);
    try {
      const res = await apiFetch<any>(`/comments/${contentId}`);
      const lista = res.data?.data ?? [];
      setComentarios(lista);
    } catch (_) {
      // Silenciamos errores
    } finally {
      setCargando(false);
    }
  };

  useEffect(() => {
    cargarComentarios();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [contentId]);

  const enviarComentario = async (e: FormEvent) => {
    e.preventDefault();
    if (!token || !nuevoComentario.trim()) return;

    try {
      const res = await apiFetch<{ id: number }>(`/comments/${contentId}`, {
        method: "POST",
        body: JSON.stringify({ body: nuevoComentario }),
      });

      // Añadimos el nuevo comentario al principio
      const comentarioCreado: Comment = {
        id: res.data.id,
        user_id: res.data.user_id,
        body: res.data.body ?? nuevoComentario,
        user: res.data.user,
      } as any;

      setComentarios((prev) => [comentarioCreado, ...prev]);
      setNuevoComentario("");
    } catch (_) {
      // Silenciar
    }
  };

  return (
    <div className={`${styles.seccionComentarios} bloque`}>
      {cargando ? (
        <p>Cargando comentarios…</p>
      ) : comentarios.length === 0 ? (
        <p>Sin comentarios.</p>
      ) : (
        <ul className={styles.listaComentarios}>
          {comentarios.map((c) => (
            <li key={c.id}>
              <strong>@{c.user?.username ?? c.user_id}:</strong> {c.body}
            </li>
          ))}
        </ul>
      )}

      {token && (
        <form onSubmit={enviarComentario} className={styles.formularioComentario}>
          <input
            type="text"
            placeholder="Escribe un comentario…"
            value={nuevoComentario}
            onChange={(e) => setNuevoComentario(e.target.value)}
          />
          <button type="submit">Enviar</button>
        </form>
      )}
    </div>
  );
} 