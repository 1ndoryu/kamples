/*
 * Hook: useHistorialIds — Kamples
 * Carga y cachea el conjunto de IDs de samples ya reproducidos.
 * Se usa para el filtro "Ya reproducidos" en el feed.
 */

import { useEffect, useState, useRef } from 'react';
import { obtenerHistorial } from '@app/services/api/varios-3/apiReproduciones';
import { crearLogger } from '@app/services/varios-1/logger';
import { useAuthStore } from '@app/stores/auth/authStore';

const log = crearLogger('useHistorialIds');
const LS_KEY_TOKEN = 'kamples_auth_token';

interface UseHistorialIdsResult {
    idsReproducidos: Set<number>;
    cargando: boolean;
}

/*
 * Carga todas las páginas del historial de reproducción
 * y devuelve un Set con los IDs de samples ya escuchados.
 * Solo carga una vez por montaje (se cachea en ref).
 */
export const useHistorialIds = (activo: boolean): UseHistorialIdsResult => {
    const [idsReproducidos, setIdsReproducidos] = useState<Set<number>>(new Set());
    const [cargando, setCargando] = useState(false);
    const cargadoRef = useRef(false);
    const autenticado = useAuthStore(s => s.autenticado);
    const cargandoAuth = useAuthStore(s => s.cargando);

    useEffect(() => {
        if (!activo || cargandoAuth) return;

        const token = typeof window !== 'undefined'
            ? window.localStorage.getItem(LS_KEY_TOKEN)
            : null;

        if (!autenticado || !token) {
            cargadoRef.current = false;
            setIdsReproducidos(new Set());
            setCargando(false);
            return;
        }

        if (cargadoRef.current) return;

        let cancelado = false;

        const cargar = async () => {
            setCargando(true);
            try {
                const ids = new Set<number>();
                let pagina = 1;
                let continuar = true;

                /* Cargar todas las páginas del historial
                 * apiGet auto-unwrap: json.data ya es SampleResumen[] directamente */
                while (continuar && !cancelado) {
                    const resp = await obtenerHistorial(pagina, 100);
                    if (cancelado) return;
                    const lista = resp.ok && Array.isArray(resp.data) ? resp.data : [];
                    if (lista.length > 0) {
                        lista.forEach((s) => ids.add(s.id));
                        if (lista.length < 100) {
                            continuar = false;
                        } else {
                            pagina++;
                        }
                    } else {
                        continuar = false;
                    }
                }

                if (!cancelado) {
                    setIdsReproducidos(ids);
                    cargadoRef.current = true;
                }
            } catch (err) {
                if (!cancelado) {
                    log.error('Error cargando historial de reproducciones', err);
                }
            }
            if (!cancelado) setCargando(false);
        };

        cargar();

        return () => { cancelado = true; };
    }, [activo, autenticado, cargandoAuth]);

    return { idsReproducidos, cargando };
};
