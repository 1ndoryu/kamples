'use client';

import styles from './SampleCard.module.css';
import {isDebug} from '../lib/debug';
import {useState, useEffect} from 'react';
import {apiFetch} from '../lib/api';
import {useInView} from '../hooks/useInView';
import LikeButton from './LikeButton';
import CommentsSection from './CommentsSection';
import {useAuthStore} from '../store/authStore';
import Waveform from '@/app/components/audio/Waveform';
import { MenuContainer, MenuButton, Menu, MenuItem } from './Menu';

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

    // --- Metadatos a mostrar (máx. 5) -------------------------------------
    const metaItems: string[] = [];
    if (sample?.content_data) {
        const cd = sample.content_data;

        // 1) Tipo
        if (cd.tipo) metaItems.push(String(cd.tipo));

        // 2) Primer género
        const generoSource = cd.genero;
        const generoFirst = Array.isArray(generoSource) ? generoSource[0] : generoSource;
        if (generoFirst && !metaItems.includes(generoFirst)) metaItems.push(String(generoFirst));

        // 3) Primer instrumento
        const instrumentoSource = cd.instrumentos;
        const instrumentoFirst = Array.isArray(instrumentoSource) ? instrumentoSource[0] : instrumentoSource;
        if (instrumentoFirst && !metaItems.includes(instrumentoFirst)) metaItems.push(String(instrumentoFirst));

        // 4) Tags (evitar duplicados y el género ya mostrado)
        const tagsList: string[] = Array.isArray(cd.tags) ? cd.tags : [];
        for (const tag of tagsList) {
            if (metaItems.length >= 5) break;
            if (!metaItems.includes(tag) && tag !== generoFirst) metaItems.push(String(tag));
        }
    }
    // ----------------------------------------------------------------------

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
            <div className={styles.imagenWrapper}>
                <img src={coverImageUrl || '/window.svg'} alt="Imagen de portada" />
                {previewUrl && (
                    <div className={styles.botonOverlay}>
                        <Waveform idSample={sample.id} urlAudio={previewUrl} soloBoton />
                    </div>
                )}
            </div>
            <div style={{display: 'flex', flexDirection: 'column', gap: 0}}>
                <h3>{sample.content_data?.title ?? sample.slug}</h3>
                <div className={styles.metaContainer}>
                    {metaItems.map(item => (
                        <span key={item} className={styles.metaBadge}>
                            {item}
                        </span>
                    ))}
                </div>
            </div>

            {/* Forma de onda completa */}
            {previewUrl && <Waveform idSample={sample.id} urlAudio={previewUrl} ocultarBoton />}

            {isDebug && originalUrl && (
                <div style={{marginTop: 8}}>
                    <p style={{fontSize: 10, opacity: 0.7, margin: 0}}>Audio original</p>
                    <audio controls src={originalUrl}>
                        Tu navegador no soporta la reproducción de audio.
                    </audio>
                </div>
            )}

            <div style={{display: 'flex', alignItems: 'center', gap: 8}}>
                <LikeButton contentId={sample.id} enabled={inView} />
                <details onToggle={e => setMostrarComentarios(e.currentTarget.open)}>
                    <summary>Comentarios</summary>
                    {mostrarComentarios && inView && <CommentsSection contentId={sample.id} />}
                </details>
                {puedeBorrar && (
                    <MenuContainer>
                        <MenuButton>
                            <svg 
                                data-testid="geist-icon" 
                                height="16" 
                                strokeLinejoin="round" 
                                viewBox="0 0 16 16" 
                                width="16" 
                                style={{color: 'currentcolor'}}
                            >
                                <path 
                                    fillRule="evenodd" 
                                    clipRule="evenodd" 
                                    d="M4 8C4 8.82843 3.32843 9.5 2.5 9.5C1.67157 9.5 1 8.82843 1 8C1 7.17157 1.67157 6.5 2.5 6.5C3.32843 6.5 4 7.17157 4 8ZM9.5 8C9.5 8.82843 8.82843 9.5 8 9.5C7.17157 9.5 6.5 8.82843 6.5 8C6.5 7.17157 7.17157 6.5 8 6.5C8.82843 6.5 9.5 7.17157 9.5 8ZM13.5 9.5C14.3284 9.5 15 8.82843 15 8C15 7.17157 14.3284 6.5 13.5 6.5C12.6716 6.5 12 7.17157 12 8C12 8.82843 12.6716 9.5 13.5 9.5Z" 
                                    fill="currentColor"
                                />
                            </svg>
                        </MenuButton>
                        <Menu width={150}>
                            <MenuItem onClick={handleDelete} type="error">Eliminar</MenuItem>
                        </Menu>
                    </MenuContainer>
                )}
            </div>

            {renderDebugInfo()}
        </article>
    );
}
