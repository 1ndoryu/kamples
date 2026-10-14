/*
 * Estado del buscador de canciones: query con debounce, resultados, carga y apertura
 * de la lista (cierre al hacer click fuera). Extraído de BuscadorCanciones sin cambio
 * de comportamiento.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { buscarCanciones } from '@app/services/api/varios-1/apiCanciones';
import type { Cancion } from '@app/types/varios-1/cancion';

export function useBuscadorCanciones(onSeleccionar: (cancion: Cancion | null) => void) {
    const [query, setQuery] = useState('');
    const [resultados, setResultados] = useState<Cancion[]>([]);
    const [cargando, setCargando] = useState(false);
    const [abierto, setAbierto] = useState(false);
    const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const contenedorRef = useRef<HTMLDivElement>(null);

    /* Cierra la lista al hacer click fuera */
    useEffect(() => {
        const manejarClickFuera = (e: MouseEvent) => {
            if (contenedorRef.current && !contenedorRef.current.contains(e.target as Node)) {
                setAbierto(false);
            }
        };
        document.addEventListener('mousedown', manejarClickFuera);
        return () => document.removeEventListener('mousedown', manejarClickFuera);
    }, []);

    /* Busqueda con debounce de 350ms */
    const buscar = useCallback((texto: string) => {
        if (timeoutRef.current) clearTimeout(timeoutRef.current);

        if (texto.length < 2) {
            setResultados([]);
            setAbierto(false);
            return;
        }

        timeoutRef.current = setTimeout(async () => {
            setCargando(true);
            try {
                const resp = await buscarCanciones(texto, 8);
                if (resp.ok && resp.data) {
                    setResultados(resp.data);
                    setAbierto(true);
                }
            } finally {
                setCargando(false);
            }
        }, 350);
    }, []);

    useEffect(() => {
        buscar(query);
        return () => {
            if (timeoutRef.current) clearTimeout(timeoutRef.current);
        };
    }, [query, buscar]);

    const elegirCancion = useCallback(
        (c: Cancion) => {
            onSeleccionar(c);
            setQuery('');
            setResultados([]);
            setAbierto(false);
        },
        [onSeleccionar]
    );

    return {
        query,
        setQuery,
        resultados,
        cargando,
        abierto,
        setAbierto,
        contenedorRef,
        elegirCancion,
    };
}
