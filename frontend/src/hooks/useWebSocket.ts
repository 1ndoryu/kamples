/* [174A-105] Hook useWebSocket() para Axum WS.
 *
 * Reemplaza al `useWebSocket` del legado que apuntaba al servidor Bun
 * standalone. Ahora habla con `/api/ws` (Axum nativo) usando el
 * flujo ticket HMAC:
 *
 *   1. GET /api/ws/ticket  → { ticket: "abc..." }   (auth requerida)
 *   2. WSS /api/ws?ticket=abc...                     (upgrade)
 *
 * Reconecta automáticamente con backoff exponencial cuando la conexión
 * cae. Renueva el ticket en cada intento porque tienen TTL corto.
 *
 * Uso:
 *   const { status, lastMessage, send } = useWebSocket({
 *     onMessage: (msg) => procesarMensaje(msg),
 *   });
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { customInstance } from '../api/axios-instance';
import { protocoloActual, ubicacionActual } from '../glory-core/platform/ventana';

export type WsStatus = 'idle' | 'connecting' | 'open' | 'closed' | 'error';

interface UseWebSocketOptions {
  enabled?: boolean;
  onMessage?: (data: unknown) => void;
  onOpen?: () => void;
  onClose?: () => void;
  /** En segundos. Backoff = min(maxBackoff, 2^retries). */
  maxBackoff?: number;
}

interface TicketResponse {
  ticket: string;
}

function resolveWsUrl(ticket: string): string {
  /* Mismo host/scheme que la app, swap http(s) → ws(s). */
  const proto = protocoloActual() === 'https:' ? 'wss:' : 'ws:';
  const host = ubicacionActual().host;
  return `${proto}//${host}/api/ws?ticket=${encodeURIComponent(ticket)}`;
}

async function fetchTicket(): Promise<string> {
  const res = await customInstance<{ data: TicketResponse }>('/api/ws/ticket', {
    method: 'GET',
  });
  /* `customInstance` devuelve `{ data, status, headers }` o el body crudo
   * según implementación; cubrimos ambos para no asumir. */
  const payload = res as { data?: { ticket?: string }; ticket?: string } | undefined;
  const ticket: string | undefined = payload?.data?.ticket ?? payload?.ticket;
  if (!ticket) throw new Error('Respuesta sin ticket');
  return ticket;
}

export function useWebSocket(options: UseWebSocketOptions = {}) {
  const { enabled = true, onMessage, onOpen, onClose, maxBackoff = 30 } = options;
  const [status, setStatus] = useState<WsStatus>('idle');
  const [lastMessage, setLastMessage] = useState<unknown>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const retriesRef = useRef(0);
  const reconnectTimerRef = useRef<number | null>(null);
  const cancelledRef = useRef(false);

  const send = useCallback((data: unknown) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      const payload = typeof data === 'string' ? data : JSON.stringify(data);
      wsRef.current.send(payload);
      return true;
    }
    return false;
  }, []);

  const connect = useCallback(async () => {
    if (cancelledRef.current) return;
    setStatus('connecting');
    try {
      const ticket = await fetchTicket();
      const ws = new WebSocket(resolveWsUrl(ticket));
      wsRef.current = ws;

      ws.onopen = () => {
        retriesRef.current = 0;
        setStatus('open');
        onOpen?.();
      };
      ws.onmessage = (ev) => {
        let data: unknown = ev.data;
        if (typeof ev.data === 'string') {
          try { data = JSON.parse(ev.data); } catch { /* deja string crudo */ }
        }
        setLastMessage(data);
        onMessage?.(data);
      };
      ws.onerror = () => {
        setStatus('error');
      };
      ws.onclose = () => {
        setStatus('closed');
        onClose?.();
        if (cancelledRef.current) return;
        const backoff = Math.min(maxBackoff, 2 ** retriesRef.current);
        retriesRef.current += 1;
        reconnectTimerRef.current = window.setTimeout(() => {
          void connect();
        }, backoff * 1000);
      };
    } catch {
      setStatus('error');
      if (cancelledRef.current) return;
      const backoff = Math.min(maxBackoff, 2 ** retriesRef.current);
      retriesRef.current += 1;
      reconnectTimerRef.current = window.setTimeout(() => {
        void connect();
      }, backoff * 1000);
    }
  }, [maxBackoff, onClose, onMessage, onOpen]);

  useEffect(() => {
    if (!enabled) return;
    cancelledRef.current = false;
    void connect();
    return () => {
      cancelledRef.current = true;
      if (reconnectTimerRef.current) {
        window.clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, [enabled, connect]);

  return { status, lastMessage, send };
}
