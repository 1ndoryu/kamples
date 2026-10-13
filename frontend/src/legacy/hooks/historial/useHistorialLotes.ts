/* sentinel-disable-file directory-size — hook legacy existente; 284A-1 agrega guardado de config sin mover el arbol completo de hooks. */
/*
 * Hook: useHistorialLotes — [223A-3] Estado y gestión del historial de lotes automáticos.
 * Carga estado de automatización + historial paginado + reactivación.
 */

import { useState, useEffect, useCallback } from 'react';
import {
    actualizarConfigProceso,
    obtenerEstadoAutomatizacion,
    obtenerHistorialLotes,
    reactivarProceso,
} from '../../services/api/varios-1/apiAutomatizacion';
import { iniciarProceso, listarProcesos, type EstadoProceso } from '../../services/api/varios-2/apiProcesos';
import type {
    AutomatizacionConfigProceso,
    EstadoAutomatizacion,
    LoteResumen,
    TipoProceso,
} from '../../services/api/varios-1/apiAutomatizacion';
import { toast } from '../../stores/varios-4/toastStore';

export function useHistorialLotes() {
    const [estado, setEstado] = useState<EstadoAutomatizacion | null>(null);
    const [procesos, setProcesos] = useState<Partial<Record<TipoProceso, EstadoProceso>>>({});
    const [lotes, setLotes] = useState<LoteResumen[]>([]);
    const [total, setTotal] = useState(0);
    const [pagina, setPagina] = useState(1);
    const [filtroTipo, setFiltroTipo] = useState<TipoProceso | ''>('');
    const [cargando, setCargando] = useState(false);
    const [guardandoConfig, setGuardandoConfig] = useState<TipoProceso | null>(null);
    const [forzandoProceso, setForzandoProceso] = useState<TipoProceso | null>(null);
    const [error, setError] = useState('');

    const cargarEstado = useCallback(async () => {
        try {
            const resp = await obtenerEstadoAutomatizacion();
            if (resp.ok && resp.data?.estado) {
                setEstado(resp.data.estado);
            }
        } catch {
            /* silencioso — estado inicial null */
        }
    }, []);

    const cargarProcesos = useCallback(async () => {
        try {
            const resp = await listarProcesos();
            if (resp.ok && resp.data?.procesos) {
                const mapaProcesos = resp.data.procesos.reduce<Partial<Record<TipoProceso, EstadoProceso>>>((acc, proceso) => {
                    if (proceso.nombre === 'extraccion' || proceso.nombre === 'scraping') {
                        acc[proceso.nombre] = proceso;
                    }
                    return acc;
                }, {});
                setProcesos(mapaProcesos);
            }
        } catch {
            /* silencioso — no bloquea historial/config */
        }
    }, []);

    const cargarHistorial = useCallback(async () => {
        setCargando(true);
        setError('');
        try {
            const tipo = filtroTipo || undefined;
            const resp = await obtenerHistorialLotes(tipo, pagina);
            if (resp.ok && resp.data) {
                setLotes(resp.data.items);
                setTotal(resp.data.total);
            } else {
                setError(resp.error ?? 'Error cargando historial');
            }
        } catch {
            setError('Error de conexión');
        } finally {
            setCargando(false);
        }
    }, [filtroTipo, pagina]);

    const manejarReactivar = useCallback(async (tipo: TipoProceso) => {
        try {
            const resp = await reactivarProceso(tipo);
            if (resp.ok) {
                await cargarEstado();
                await cargarHistorial();
                await cargarProcesos();
                toast.exito(resp.data?.mensaje ?? 'Proceso reactivado');
            } else {
                toast.error(resp.error ?? 'No se pudo reactivar el proceso');
            }
            return resp;
        } catch {
            toast.error('Error de conexión');
            return { ok: false, error: 'Error de conexión' };
        }
    }, [cargarEstado, cargarHistorial, cargarProcesos]);

    const forzarProceso = useCallback(async (tipo: TipoProceso, limite?: number) => {
        setForzandoProceso(tipo);
        try {
            const resp = await iniciarProceso(tipo, limite);
            if (resp.ok && !resp.data?.error) {
                await cargarEstado();
                await cargarHistorial();
                await cargarProcesos();
                toast.exito(resp.data?.mensaje ?? 'Ejecución iniciada');
            } else {
                toast.error(resp.data?.error ?? resp.error ?? 'No se pudo iniciar el proceso');
            }
            return resp;
        } catch {
            toast.error('Error de conexión');
            return { ok: false, error: 'Error de conexión' };
        } finally {
            setForzandoProceso(null);
        }
    }, [cargarEstado, cargarHistorial, cargarProcesos]);

    const guardarConfig = useCallback(async (
        tipo: TipoProceso,
        config: Required<AutomatizacionConfigProceso>
    ) => {
        setGuardandoConfig(tipo);
        try {
            const resp = await actualizarConfigProceso(tipo, config);
            if (resp.ok) {
                await cargarEstado();
                await cargarHistorial();
                await cargarProcesos();
                toast.exito('Configuración actualizada');
            } else {
                toast.error(resp.error ?? 'No se pudo actualizar la configuración');
            }
            return resp;
        } catch {
            toast.error('Error de conexión');
            return { ok: false, error: 'Error de conexión' };
        } finally {
            setGuardandoConfig(null);
        }
    }, [cargarEstado, cargarHistorial, cargarProcesos]);

    const refrescarTodo = useCallback(async () => {
        await Promise.all([cargarEstado(), cargarHistorial(), cargarProcesos()]);
    }, [cargarEstado, cargarHistorial, cargarProcesos]);

    useEffect(() => {
        refrescarTodo();
    }, [refrescarTodo]);

    return {
        estado,
        procesos,
        lotes,
        total,
        pagina,
        setPagina,
        filtroTipo,
        setFiltroTipo,
        cargando,
        guardandoConfig,
        forzandoProceso,
        error,
        refrescar: refrescarTodo,
        refrescarEstado: cargarEstado,
        reactivar: manejarReactivar,
        forzarProceso,
        guardarConfig,
    };
}
