/*
 * Hook: useWebSocket — Kamples (QK68)
 * Gestiona el ciclo de vida de la conexión WebSocket:
 * - Conecta después de autenticación exitosa (con ticket HMAC)
 * - Desconecta al cerrar sesión
 * - Renueva ticket antes de expiración
 * Se monta una vez en InicializadorAuth.
 */

import { useEffect, useState, useCallback, useRef } from 'react';
import { useAuthStore } from '@app/stores/auth/authStore';
import { wsService } from '@app/services/varios-2/wsService';
import { apiGet } from '@app/services/api/varios-1/apiCliente';
import { crearLogger } from '@app/services/varios-1/logger';
import type { HandlerMensaje, EstadoConexion } from '@app/services/varios-2/wsService';

const log = crearLogger('useWebSocket');

/* Intervalo de renovación de ticket: 90s (ticket dura 120s) */
const RENOVACION_TICKET_MS = 90_000;

interface RespuestaTicket {
    ticket: string;
    url: string | null;
    ttl: number;
}

async function obtenerTicketWS(): Promise<RespuestaTicket | null> {
    const resp = await apiGet<RespuestaTicket>('/ws/ticket');
    if (resp.ok && resp.data) {
        return resp.data;
    }
    return null;
}

export const useWebSocket = () => {
    const [estadoConexion, setEstadoConexion] = useState<EstadoConexion>(
        wsService.obtenerEstado()
    );
    const autenticado = useAuthStore(s => s.autenticado);
    const usuario = useAuthStore(s => s.usuario);
    const renovacionRef = useRef<number | null>(null);

    /* Suscribir handler de tipo específico, se limpia al desmontar */
    const on = useCallback((tipo: string, handler: HandlerMensaje) => {
        return wsService.on(tipo, handler);
    }, []);

    /* Enviar mensaje */
    const enviar = useCallback((tipo: string, datos: unknown = {}) => {
        return wsService.enviar(tipo, datos);
    }, []);

    /* Sincronizar estado reactivo con wsService */
    useEffect(() => {
        const unsub = wsService.onCambioEstado(setEstadoConexion);
        return unsub;
    }, []);

    /* Conectar/desconectar basado en auth */
    useEffect(() => {
        if (!autenticado || !usuario?.id) {
            wsService.desconectar();
            if (renovacionRef.current) {
                clearInterval(renovacionRef.current);
                renovacionRef.current = null;
            }
            return;
        }

        let cancelado = false;

        const conectar = async () => {
            try {
                const ticketData = await obtenerTicketWS();
                if (cancelado) return;

                if (!ticketData?.ticket || !ticketData.url) {
                    log.debug('WS ticket no disponible (servidor no configurado)');
                    return;
                }

                wsService.configurar(ticketData.url, usuario.id, ticketData.ticket);
                wsService.conectar();
                log.info('WS conectando', { userId: usuario.id });

                /* Renovar ticket periódicamente para reconexiones */
                renovacionRef.current = window.setInterval(async () => {
                    try {
                        const nuevoTicket = await obtenerTicketWS();
                        if (nuevoTicket?.ticket) {
                            wsService.actualizarTicket(nuevoTicket.ticket);
                        }
                    } catch {
                        log.debug('Error renovando ticket WS');
                    }
                }, RENOVACION_TICKET_MS);
            } catch {
                log.debug('Error obteniendo ticket WS');
            }
        };

        conectar();

        return () => {
            cancelado = true;
            wsService.desconectar();
            if (renovacionRef.current) {
                clearInterval(renovacionRef.current);
                renovacionRef.current = null;
            }
        };
    }, [autenticado, usuario?.id]);

    return {
        estadoConexion,
        conectado: estadoConexion === 'conectado',
        enviar,
        on,
    };
};
