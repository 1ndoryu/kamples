'use client';
import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { apiFetch, API_URL } from '../lib/api';
import { useAuthStore } from '../store/authStore';
import { useUiStore } from '../store/uiStore';

export default function PublicarForm() {
  const { token } = useAuthStore();
  const closeModal = useUiStore((s) => s.closeModal);
  const router = useRouter();

  const [slug, setSlug] = useState('');
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [audioFile, setAudioFile] = useState<File | null>(null);
  const [imageFile, setImageFile] = useState<File | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  if (!token) {
    return <p>Debes iniciar sesión para publicar.</p>;
  }

  // Sube un archivo y devuelve el id que responde la API
  async function uploadFile(file: File): Promise<number> {
    const form = new FormData();
    form.append('file', file);

    const headers: Record<string, string> = {};
    const savedToken = typeof window !== 'undefined' ? localStorage.getItem('token') : null;
    if (savedToken) headers['Authorization'] = `Bearer ${savedToken}`;

    const res = await fetch(`${API_URL}/media`, {
      method: 'POST',
      headers,
      body: form,
    });

    const json = await res.json();
    if (!res.ok) throw new Error(json?.message || 'Error al subir archivo');
    // Suponemos que la API devuelve {data: {id: number}}
    const id = json.data?.id;
    if (!id) throw new Error('La respuesta de /media no contiene id');
    return id;
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      let audioMediaId: number | null = null;
      let imageMediaId: number | null = null;

      if (audioFile) audioMediaId = await uploadFile(audioFile);
      if (imageFile) imageMediaId = await uploadFile(imageFile);

      const res = await apiFetch<any>('/contents', {
        method: 'POST',
        body: JSON.stringify({
          slug,
          type: 'audio_sample',
          status: 'published',
          content_data: {
            title,
            body,
            media_id: audioMediaId,
            cover_image_id: imageMediaId,
          },
        }),
      });
      setSuccess(res.message);
      setTimeout(() => {
        closeModal();
        router.push('/');
      }, 1200);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }

  return (
    <>
      <h1>Publicar Sample</h1>
      {error && <p style={{ color: 'red' }}>{error}</p>}
      {success && <p style={{ color: 'green' }}>{success}</p>}
      <form onSubmit={handleSubmit} className="bloque">
        <div className="bloque">
          <label>Slug</label>
          <input value={slug} onChange={(e) => setSlug(e.target.value)} />
        </div>
        <div className="bloque">
          <label>Título</label>
          <input value={title} onChange={(e) => setTitle(e.target.value)} />
        </div>
        <div className="bloque">
          <label>Descripción</label>
          <textarea value={body} onChange={(e) => setBody(e.target.value)} />
        </div>
        <div className="bloque">
          <label>Archivo de audio</label>
          <input type="file" accept="audio/*" onChange={(e) => setAudioFile(e.target.files?.[0] || null)} />
        </div>
        <div className="bloque">
          <label>Imagen de portada (opcional)</label>
          <input type="file" accept="image/*" onChange={(e) => setImageFile(e.target.files?.[0] || null)} />
        </div>
        <button type="submit" disabled={loading}>{loading ? 'Publicando…' : 'Publicar'}</button>
      </form>
    </>
  );
} 