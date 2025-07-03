import {useState, useEffect} from 'react';
import {apiFetch} from '../lib/api';
import {useAuthStore} from '../store/authStore';
import styles from './LikeButton.module.css';

interface Props {
    contentId: number;
    enabled?: boolean; // Si es false, no se hace la petición aún
}

export default function LikeButton({contentId, enabled = true}: Props) {
    const {token} = useAuthStore();
    const [cuentaLikes, setCuentaLikes] = useState<number>(0);
    const [liked, setLiked] = useState<boolean>(false);
    const [cargando, setCargando] = useState<boolean>(false);

    // Cargar likes sólo cuando enabled === true
    useEffect(() => {
        if (!enabled) return;

        async function cargarLikes() {
            try {
                const res = await apiFetch<{like_count: number; liked?: boolean}>(`/contents/${contentId}/likes`);
                setCuentaLikes(res.data.like_count);
                if (typeof res.data.liked === 'boolean') {
                    setLiked(res.data.liked);
                }
            } catch (_) {
                // Silenciamos errores
            }
        }
        cargarLikes();
    }, [contentId, enabled]);

    const alternarLike = async () => {
        if (!token || cargando) return; // Requiere autenticación
        const previoLiked = liked;
        const prevCuenta = cuentaLikes;

        const nuevoLiked = !liked;
        setLiked(nuevoLiked);
        setCuentaLikes(prev => prev + (nuevoLiked ? 1 : -1));

        setCargando(true);
        try {
            const res = await apiFetch<{like_count: number; liked?: boolean}>(`/contents/${contentId}/like`, {method: 'POST'});
            setCuentaLikes(res.data.like_count);
            if (typeof res.data.liked === 'boolean') setLiked(res.data.liked);
        } catch (_) {
            setLiked(previoLiked);
            setCuentaLikes(prevCuenta);
        } finally {
            setCargando(false);
        }
    };

    return (
        <button
            className={`${styles.botonLike} ${liked ? styles.liked : styles.notLiked}`}
            onClick={alternarLike}
            disabled={!token || cargando}
            title={token ? 'Dar me gusta' : 'Inicia sesión para dar me gusta'}
        >
            <svg data-testid="geist-icon" height="16" strokeLinejoin="round" viewBox="0 0 16 16" width="16" style={{color: 'currentcolor'}}>
                <path fillRule="evenodd" clipRule="evenodd" d="M7.06463 3.20474C5.79164 1.93175 3.72772 1.93175 2.45474 3.20474C1.18175 4.47773 1.18175 6.54166 2.45474 7.81465L8 13.3599L13.5453 7.81465C14.8182 6.54166 14.8182 4.47773 13.5453 3.20474C12.2723 1.93175 10.2084 1.93175 8.93537 3.20474L8.53033 3.60979L8 4.14012L7.46967 3.60979L7.06463 3.20474ZM8 2.02321C6.13348 0.286219 3.21165 0.326509 1.39408 2.14408C-0.464694 4.00286 -0.464691 7.01653 1.39408 8.87531L7.46967 14.9509L8 15.4812L8.53033 14.9509L14.6059 8.87531C16.4647 7.01653 16.4647 4.00286 14.6059 2.14408C12.7884 0.326509 9.86653 0.286221 8 2.02321Z" fill="currentColor"></path>
            </svg>{' '}
            {cuentaLikes}
        </button>
    );
}
