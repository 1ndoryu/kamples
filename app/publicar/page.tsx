'use client';
import {useState} from 'react';
import {useRouter} from 'next/navigation';
import {apiFetch} from '../../lib/api';
import {useAuthStore} from '../../store/authStore';

export default function PublicarPage() {
    const {token} = useAuthStore();
    const router = useRouter();

    const [slug, setSlug] = useState('');
    const [title, setTitle] = useState('');
    const [body, setBody] = useState('');
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState<string | null>(null);

    if (!token) {
        return <p>Debes iniciar sesión para publicar.</p>;
    }

    async function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        try {
            const res = await apiFetch<any>('/contents', {
                method: 'POST',
                body: JSON.stringify({
                    slug,
                    type: 'audio_sample',
                    status: 'published',
                    content_data: {
                        title,
                        body
                    }
                })
            });
            setSuccess(res.message);
            // Redirigir a inicio después de 1 segundo
            setTimeout(() => router.push('/'), 1000);
        } catch (e: any) {
            setError(e.message);
        }
    }

    return (
        <div style={{padding: '32px'}}>
            <h1>Publicar Sample</h1>
            {error && <p style={{color: 'red'}}>{error}</p>}
            {success && <p style={{color: 'green'}}>{success}</p>}
            <form onSubmit={handleSubmit} className="bloque">
                <div className="bloque">
                    <label>Slug</label>
                    <input value={slug} onChange={e => setSlug(e.target.value)} />
                </div>
                <div className="bloque">
                    <label>Título</label>
                    <input value={title} onChange={e => setTitle(e.target.value)} />
                </div>
                <div className="bloque">
                    <label>Descripción</label>
                    <textarea value={body} onChange={e => setBody(e.target.value)} />
                </div>
                <button type="submit">Publicar</button>
            </form>
        </div>
    );
}
