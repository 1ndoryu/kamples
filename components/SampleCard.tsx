'use client';

import styles from './SampleCard.module.css';
import {isDebug} from '../lib/debug';
import {useState, useEffect} from 'react';
import {apiFetch} from '../lib/api';
import {useInView} from '../hooks/useInView';
import LikeButton from './LikeButton';
import CommentsSection from './CommentsSection';
import {useAuthStore} from '../store/authStore';
import Avatar from './Avatar';
import Waveform from '@/app/components/audio/Waveform';

interface Props {
    sample: any;
    onDeleted?: (id: number) => void;
}

export default function SampleCard({sample, onDeleted}: Props) {
    const [expandido, setExpandido] = useState(false);
    const [mostrarComentarios, setMostrarComentarios] = useState(false);
    const [cardRef, inView] = useInView<HTMLElement>({threshold: 0.25});
    const lightMediaId: string | number | null = sample?.content_data?.light_media_id ?? null;
    const fallbackMediaId: string | number | null = sample?.content_data?.media_id ?? null;
    const mediaId: string | number | null = lightMediaId ?? fallbackMediaId;
    const originalMediaId: string | number | null = sample?.content_data?.original_media_id ?? fallbackMediaId;
    const coverImageId: string | number | null = sample?.content_data?.cover_image_id ?? null;
    const [previewUrl, setPreviewUrl] = useState<string | null>(null);
    const [originalUrl, setOriginalUrl] = useState<string | null>(null);
    const [coverImageUrl, setCoverImageUrl] = useState<string | null>(null);
    const [eliminado, setEliminado] = useState(false);
    const currentUser = useAuthStore(s => s.user);
    const puedeBorrar = currentUser && currentUser.id === sample.user_id;

    const handleDelete = async () => {
        if (!confirm('¿Seguro que deseas borrar este sample? Esta acción es irreversible.')) return;

        try {
            await apiFetch<null>(`/contents/${sample.id}`, {method: 'DELETE'});
            if (typeof onDeleted === 'function') {
                onDeleted(sample.id);
            } else {
                setEliminado(true);
            }
        } catch (e: any) {
            alert(e.message || 'Error eliminando el sample');
        }
    };

    useEffect(() => {
        if (!mediaId || !inView) return;

        (async () => {
            try {
                const res = await apiFetch<any>(`/media/${mediaId}`);
                const path = res?.data?.path;
                if (path) setPreviewUrl(`/api/${path}`);
            } catch (e) {
                if (isDebug) console.error('Error obteniendo media preview', e);
            }
        })();
    }, [mediaId, inView]);

    useEffect(() => {
        if (!isDebug || !originalMediaId || !inView) return;

        (async () => {
            try {
                const res = await apiFetch<any>(`/media/${originalMediaId}`);
                const path = res?.data?.path;
                if (path) setOriginalUrl(`/api/${path}`);
            } catch (e) {
                if (isDebug) console.error('Error obteniendo media original', e);
            }
        })();
    }, [originalMediaId, inView]);

    useEffect(() => {
        if (!coverImageId || !inView) return;

        (async () => {
            try {
                const res = await apiFetch<any>(`/media/${coverImageId}`);
                const path = res?.data?.path;
                if (path) setCoverImageUrl(`/api/${path}`);
            } catch (e) {
                if (isDebug) console.error('Error obteniendo cover image', e);
            }
        })();
    }, [coverImageId, inView]);

    const renderDebugInfo = () => {
        if (!isDebug || !sample.content_data) return null;

        const entradas = Object.entries(sample.content_data).sort((a, b) => a[0].localeCompare(b[0]));

        const entradasMostradas = expandido ? entradas : entradas.slice(0, 5);

        return (
            <div className={styles.debugInfo}>
                <ul>
                    {entradasMostradas.map(([clave, valor]) => (
                        <li key={clave}>
                            <strong>{clave}:</strong> {Array.isArray(valor) ? valor.join(', ') : String(valor)}
                        </li>
                    ))}
                </ul>

                {isDebug && previewUrl && (
                    <>
                        <strong>URL:</strong> {previewUrl}
                    </>
                )}

                {entradas.length > 5 && (
                    <button type="button" onClick={() => setExpandido(!expandido)} className={styles.botonExpandir}>
                        {expandido ? 'Ocultar' : 'Expandir'}
                    </button>
                )}
            </div>
        );
    };

    if (eliminado) return null;

    return (
        <article ref={cardRef} className={`${styles.tarjetaSample}`}>
            {coverImageUrl && <img className="img-sample" src={coverImageUrl} alt="Imagen de portada" style={{maxWidth: '40px', aspectRatio: '1 / 1', objectFit: 'cover', borderRadius: '5px'}} />}
            <h3>{sample.content_data?.title ?? sample.slug}</h3>
            <div style={{display: 'flex', alignItems: 'center', gap: 8}}>
                {/* <Avatar userId={sample.user_id} size={32} /> */}
                {/*<span style={{fontSize: 14}}>Publicado por usuario #{sample.user_id}</span>*/}
            </div>
            {previewUrl && <Waveform idSample={sample.id} urlAudio={previewUrl} />}

            {isDebug && originalUrl && (
                <div style={{marginTop: 8}}>
                    <p style={{fontSize: 10, opacity: 0.7, margin: 0}}>Audio original</p>
                    <audio controls src={originalUrl}>
                        Tu navegador no soporta la reproducción de audio.
                    </audio>
                </div>
            )}

            <div style={{marginTop: 8, display: 'flex', alignItems: 'center', gap: 8}}>
                <LikeButton contentId={sample.id} enabled={inView} />
                <details onToggle={e => setMostrarComentarios(e.currentTarget.open)}>
                    <summary>Comentarios</summary>
                    {mostrarComentarios && inView && <CommentsSection contentId={sample.id} />}
                </details>
                {puedeBorrar && (
                    <button type="button" onClick={handleDelete} style={{background: 'transparent', border: 'none', color: 'red', cursor: 'pointer'}} title="Eliminar sample">
                        🗑️
                    </button>
                )}
            </div>

            {renderDebugInfo()}
        </article>
    );
}
