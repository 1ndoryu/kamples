/*
 * Hook: useCerrarAlClickFuera — Kamples
 * Cierra un dropdown al pulsar fuera de su contenedor.
 * Extraído de BarraControlFeed y FiltroColumna, que tenían el mismo listener duplicado.
 * El listener solo existe mientras `abierto`: no queda ningún handler global inactivo.
 */

import { useEffect, type RefObject } from 'react';

export function useCerrarAlClickFuera(
    ref: RefObject<HTMLElement>,
    abierto: boolean,
    onCerrar: () => void,
): void {
    useEffect(() => {
        if (!abierto) return;
        const manejarClickFuera = (e: MouseEvent) => {
            if (ref.current && !ref.current.contains(e.target as Node)) {
                onCerrar();
            }
        };
        document.addEventListener('mousedown', manejarClickFuera);
        return () => document.removeEventListener('mousedown', manejarClickFuera);
    }, [abierto, ref, onCerrar]);
}
