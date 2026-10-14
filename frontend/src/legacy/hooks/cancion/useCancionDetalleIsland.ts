/*
 * Estado de interacción de CancionDetalleIsland: proveedor de samples de la canción,
 * like con rollback optimista. Extraído del componente sin cambio de comportamiento.
 */

import { useCallback, useEffect, useState } from 'react';
import { obtenerSamplesDeCancion } from '@app/services/api/varios-3/apiSamples';
import type { SampleResumen } from '@app/types';
import { darLike, quitarLike } from '@app/services/api/varios-3/apiSocial';
import type { CancionDetalle } from '@app/types/varios-1/cancion';

interface UseCancionDetalleIslandParams {
    slug?: string;
    detalle: CancionDetalle | null | undefined;
    autenticado: boolean;
}

export function useCancionDetalleIsland({ slug, detalle, autenticado }: UseCancionDetalleIslandParams) {
    /* Proveedor de samples extraídos de esta canción (cancion_origen_id) */
    const proveedorSamples = useCallback(
        (_pagina: number) =>
            obtenerSamplesDeCancion(slug ?? '')
                .then((r) => ({ ok: r.ok, data: r.ok && r.data ? r.data : [] as SampleResumen[] }))
                .catch(() => ({ ok: false, data: [] as SampleResumen[] })),
        [slug]
    );

    /* [183A-32] Like de canción con rollback optimista */
    const [liked, setLiked] = useState(detalle?.cancion?.liked ?? false);
    const [likeando, setLikeando] = useState(false);

    /* Sincronizar liked cuando cambia la canción cargada */
    useEffect(() => {
        if (detalle?.cancion) setLiked(detalle.cancion.liked ?? false);
    }, [detalle?.cancion?.id]);

    const manejarToggleLike = useCallback(async () => {
        if (!autenticado || likeando || !detalle) return;
        const anterior = liked;
        setLikeando(true);
        setLiked(!anterior);

        const resp = anterior
            ? await quitarLike('cancion', detalle.cancion.id)
            : await darLike('cancion', detalle.cancion.id);

        /* [223A-9] Fix: backend ahora retorna { liked, reaccion } dentro de data.
         * Defensivo: si liked no viene, mantener el estado optimista. */
        if (!resp.ok) setLiked(anterior);
        else if (resp.data && typeof resp.data.liked === 'boolean') setLiked(resp.data.liked);
        setLikeando(false);
    }, [autenticado, likeando, liked, detalle]);

    return { liked, likeando, proveedorSamples, manejarToggleLike };
}
