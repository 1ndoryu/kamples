/*
 * Hook: useChatIsland — Kamples
 * [183A-62] Lógica de conversación individual: cargar mensajes con cursor-based pagination.
 * Carga inicial: últimos 50 mensajes. Scroll arriba: cargar más antiguos.
 * Cleanup con activo=false en carga de mensajes.
 */

import { useEffect, useRef, useState, useCallback } from 'react';
import {
    obtenerMensajes,
    obtenerConversaciones,
    enviarMensaje,
    enviarMensajeMultimedia,
    marcarConversacionLeida,
} from '@app/services/api/varios-2/apiMensajes';
import { useMensajesStore } from '@app/stores/varios-2/mensajesStore';
import { useAuthStore } from '@app/stores/auth/authStore';
import { useNavigationStore } from '@/core/router';
import type { Conversacion, Mensaje } from '@app/types';
import { rutaActual } from '@/platform/ventana';

/* Formatear fecha para agrupación */
const formatearFechaGrupo = (fecha: string): string => {
    const hoy = new Date();
    const msg = new Date(fecha);
    const diff = hoy.getTime() - msg.getTime();
    const dias = Math.floor(diff / (24 * 60 * 60 * 1000));

    if (dias === 0) return 'Hoy';
    if (dias === 1) return 'Ayer';
    if (dias < 7) return msg.toLocaleDateString('es', { weekday: 'long' });
    return msg.toLocaleDateString('es', { day: 'numeric', month: 'short' });
};

/* Agrupar mensajes por fecha */
export const agruparPorFecha = (mensajes: Mensaje[]): { fecha: string; mensajes: Mensaje[] }[] => {
    const grupos: Record<string, Mensaje[]> = {};
    mensajes.forEach(m => {
        const clave = new Date(m.creadoAt).toDateString();
        if (!grupos[clave]) grupos[clave] = [];
        grupos[clave].push(m);
    });
    return Object.entries(grupos).map(([, msgs]) => ({
        fecha: formatearFechaGrupo(msgs[0].creadoAt),
        mensajes: msgs,
    }));
};

interface UseChatIslandParams {
    conversacionId?: string;
}

export const useChatIsland = ({ conversacionId: propId }: UseChatIslandParams) => {
    const mensajes = useMensajesStore(s => s.mensajes);
    const conversaciones = useMensajesStore(s => s.conversaciones);
    const setMensajes = useMensajesStore(s => s.setMensajes);
    const agregarMensaje = useMensajesStore(s => s.agregarMensaje);
    const eliminarMensaje = useMensajesStore(s => s.eliminarMensaje);
    const setConversaciones = useMensajesStore(s => s.setConversaciones);
    const actualizarUltimoMensaje = useMensajesStore(s => s.actualizarUltimoMensaje);

    const usuario = useAuthStore(s => s.usuario);
    const navegar = useNavigationStore(s => s.navegar);

    const [textoMensaje, setTextoMensaje] = useState('');
    const [enviando, setEnviando] = useState(false);
    const [cargando, setCargando] = useState(true);
    const [conversacion, setConversacion] = useState<Conversacion | null>(null);
    /* [183A-62] Estado de paginación por cursor */
    const [hayMas, setHayMas] = useState(false);
    const [cargandoMas, setCargandoMas] = useState(false);

    const mensajesRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLTextAreaElement>(null);
    const archivoRef = useRef<HTMLInputElement>(null);
    /* [183A-62] Flag para evitar auto-scroll al cargar mensajes antiguos */
    const cargandoAntiguosRef = useRef(false);

    const miId = usuario?.id ?? 1;

    /* ID de conversación: prop de la ruta o el segmento que sigue a `chat`
     * (/mensajes/chat/<id>/). Se valida con Number.isInteger porque parseInt('chat')
     * devuelve NaN y NaN es falso: la ruta sin id se colaba como "sin conversación"
     * y la isla se quedaba cargando para siempre. */
    const conversacionId = (() => {
        const crudo = propId ?? rutaActual().split('/').filter(Boolean)[2] ?? '';
        const id = parseInt(crudo, 10);
        return Number.isInteger(id) && id > 0 ? id : null;
    })();

    /* Cargar conversación y mensajes más recientes — con cleanup */
    useEffect(() => {
        /* Sin id no hay nada que cargar: se apaga el cargando para que la isla pueda
         * explicarlo, en vez de dejar el esqueleto para siempre. */
        if (!conversacionId) {
            setCargando(false);
            return;
        }
        let activo = true;

        const cargar = async () => {
            setCargando(true);
            try {
                /* Carga paralela: conversaciones (si faltan) + mensajes al mismo tiempo */
                const necesitaConvs = conversaciones.length === 0;
                const [respConvs, respMsgs] = await Promise.all([
                    necesitaConvs
                        ? obtenerConversaciones()
                        : Promise.resolve(null),
                    obtenerMensajes(conversacionId),
                ]);
                if (!activo) return;

                let convs = conversaciones;
                if (respConvs?.ok && respConvs.data) {
                    setConversaciones(respConvs.data);
                    convs = respConvs.data;
                }

                const convActiva = convs.find(c => c.id === conversacionId) ?? null;
                setConversacion(convActiva);

                if (respMsgs.ok && respMsgs.data) {
                    setMensajes(respMsgs.data);
                    setHayMas(respMsgs.hayMas ?? false);
                }

                /* Fire-and-forget: marcar leida no bloquea la UI */
                if (convActiva && convActiva.noLeidos > 0) {
                    useMensajesStore.getState().marcarConversacionLeida(conversacionId);
                    marcarConversacionLeida(conversacionId);
                }
            } catch {
                /* Fallo de carga silencioso */
            } finally {
                if (activo) setCargando(false);
            }
        };
        cargar();
        return () => { activo = false; };
    }, [conversacionId, setMensajes, setConversaciones]);

    /* Auto-scroll al final solo al cargar inicial o enviar mensaje (no al cargar antiguos) */
    useEffect(() => {
        if (cargandoAntiguosRef.current) return;
        if (mensajesRef.current) {
            mensajesRef.current.scrollTop = mensajesRef.current.scrollHeight;
        }
    }, [mensajes]);

    /* [183A-62] Cargar mensajes más antiguos (scroll hacia arriba) */
    const cargarMasAntiguos = useCallback(async () => {
        if (!conversacionId || cargandoMas || !hayMas || mensajes.length === 0) return;

        const primerMensajeId = mensajes[0]?.id;
        if (!primerMensajeId) return;

        setCargandoMas(true);
        cargandoAntiguosRef.current = true;

        /* Guardar scroll height para mantener posición después de prepend */
        const contenedor = mensajesRef.current;
        const scrollHeightAntes = contenedor?.scrollHeight ?? 0;

        try {
            const resp = await obtenerMensajes(conversacionId, primerMensajeId);
            if (resp.ok && resp.data && resp.data.length > 0) {
                /* Prepend: mensajes antiguos + mensajes actuales */
                const actuales = useMensajesStore.getState().mensajes;
                setMensajes([...resp.data, ...actuales]);
                setHayMas(resp.hayMas ?? false);

                /* Restaurar posición de scroll para que no salte */
                requestAnimationFrame(() => {
                    if (contenedor) {
                        const scrollHeightDespues = contenedor.scrollHeight;
                        contenedor.scrollTop = scrollHeightDespues - scrollHeightAntes;
                    }
                    cargandoAntiguosRef.current = false;
                });
                return;
            }
            setHayMas(false);
        } catch {
            /* Error silencioso al cargar más */
        } finally {
            setCargandoMas(false);
            cargandoAntiguosRef.current = false;
        }
    }, [conversacionId, cargandoMas, hayMas, mensajes, setMensajes]);

    /* [183A-62] Detectar scroll arriba para cargar más */
    const manejarScroll = useCallback(() => {
        const contenedor = mensajesRef.current;
        if (!contenedor || cargandoMas || !hayMas) return;
        /* Cuando el scroll está cerca del tope (< 100px), cargar más */
        if (contenedor.scrollTop < 100) {
            cargarMasAntiguos();
        }
    }, [cargarMasAntiguos, cargandoMas, hayMas]);

    const manejarEnviar = useCallback(async () => {
        if (!textoMensaje.trim() || !conversacionId || enviando) return;

        const contenido = textoMensaje.trim();
        setTextoMensaje('');
        setEnviando(true);

        const mensajeOptimista: Mensaje = {
            id: Date.now(),
            conversacionId,
            remitenteId: miId,
            contenido,
            tipo: 'texto',
            leido: false,
            creadoAt: new Date().toISOString(),
        };
        agregarMensaje(mensajeOptimista);
        actualizarUltimoMensaje(conversacionId, contenido);

        try {
            await enviarMensaje(conversacionId, contenido);
        } catch {
            eliminarMensaje(mensajeOptimista.id);
        } finally {
            setEnviando(false);
        }
        inputRef.current?.focus();
    }, [textoMensaje, conversacionId, enviando, miId, agregarMensaje, eliminarMensaje, actualizarUltimoMensaje]);

    const manejarArchivo = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
        const archivo = e.target.files?.[0];
        if (!archivo || !conversacionId) return;

        const esImagen = archivo.type.startsWith('image/');
        const esAudio = archivo.type.startsWith('audio/');
        if (!esImagen && !esAudio) return;

        setEnviando(true);
        const tipo = esImagen ? 'imagen' as const : 'audio' as const;

        const msgOptimista: Mensaje = {
            id: Date.now(),
            conversacionId,
            remitenteId: miId,
            contenido: esImagen ? '[Imagen]' : '[Audio]',
            tipo,
            mediaUrl: URL.createObjectURL(archivo),
            leido: false,
            creadoAt: new Date().toISOString(),
        };
        agregarMensaje(msgOptimista);
        actualizarUltimoMensaje(conversacionId, esImagen ? '[Imagen]' : '[Audio]');

        try {
            await enviarMensajeMultimedia(conversacionId, tipo, archivo);
        } catch {
            eliminarMensaje(msgOptimista.id);
        } finally {
            setEnviando(false);
            if (archivoRef.current) archivoRef.current.value = '';
        }
    }, [conversacionId, miId, agregarMensaje, eliminarMensaje, actualizarUltimoMensaje]);

    const manejarKeyDown = useCallback(
        (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                manejarEnviar();
            }
        },
        [manejarEnviar]
    );

    const gruposMensajes = agruparPorFecha(mensajes);

    return {
        mensajes, gruposMensajes, textoMensaje, setTextoMensaje,
        enviando, cargando, conversacion, miId, conversacionId,
        hayMas, cargandoMas, manejarScroll,
        mensajesRef, inputRef, archivoRef,
        navegar, manejarEnviar, manejarArchivo, manejarKeyDown,
    };
};
