'use client';
import { useState, useEffect } from 'react';
import { apiFetch, API_URL } from '../lib/api';
import { useAuthStore } from '../store/authStore';
import { useUiStore } from '../store/uiStore';

export default function PerfilForm() {
  const { user, loadProfile } = useAuthStore();
  const closeModal = useUiStore((s) => s.closeModal);

  const [username, setUsername] = useState('');
  const [publicName, setPublicName] = useState('');
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const [avatarFile, setAvatarFile] = useState<File | null>(null);

  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // Cargar valores iniciales cuando se abra el modal
  useEffect(() => {
    if (!user) return;
    setUsername(user.username ?? '');
    setPublicName(user.profile_data?.public_name ?? '');
    setAvatarUrl(user.profile_data?.avatar_url ?? null);
  }, [user]);

  async function uploadFile(file: File): Promise<string> {
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
    const path = json.data?.path;
    if (!path) throw new Error('La respuesta de /media no contiene path');
    return path as string;
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSuccess(null);
    setLoading(true);

    try {
      let finalAvatarUrl = avatarUrl;
      if (avatarFile) {
        finalAvatarUrl = await uploadFile(avatarFile);
      }

      await apiFetch<null>('/user/profile', {
        method: 'POST',
        body: JSON.stringify({
          username,
          profile_data: {
            public_name: publicName,
            avatar_url: finalAvatarUrl,
          },
        }),
      });

      await loadProfile();
      setSuccess('Perfil actualizado.');
      setTimeout(() => {
        closeModal();
      }, 1200);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }

  return (
    <>
      <h1>Editar perfil</h1>
      {error && <p style={{ color: 'red' }}>{error}</p>}
      {success && <p style={{ color: 'green' }}>{success}</p>}
      <form onSubmit={handleSubmit} className="bloque">
        <div className="bloque">
          <label>Usuario</label>
          <input value={username} onChange={(e) => setUsername(e.target.value)} />
        </div>
        <div className="bloque">
          <label>Nombre público</label>
          <input value={publicName} onChange={(e) => setPublicName(e.target.value)} />
        </div>
        <div className="bloque">
          <label>Foto de perfil</label>
          {avatarUrl && (
            <div className="bloque" style={{ marginBottom: '8px' }}>
              <img src={`${API_URL}/${avatarUrl}`} alt="avatar" style={{ width: 80, height: 80, objectFit: 'cover', borderRadius: '50%' }} />
            </div>
          )}
          <input type="file" accept="image/*" onChange={(e) => setAvatarFile(e.target.files?.[0] || null)} />
        </div>
        <button type="submit" disabled={loading}>{loading ? 'Guardando…' : 'Guardar cambios'}</button>
      </form>
    </>
  );
} 