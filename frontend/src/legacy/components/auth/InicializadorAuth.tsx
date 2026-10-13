/*
 * Componente: InicializadorAuth
 * Se monta una sola vez en AppProvider.
 * Lee GLORY_CONTEXT para detectar sesión de WordPress y sincroniza el authStore.
 * Si hay sesión PHP, también intenta obtener datos completos del perfil Kamples via API.
 * QK68: Inicializa conexión WebSocket después de auth.
 * Notificaciones nativas: muestra en tray de Android/desktop cuando llegan eventos WS.
 * QL17: Back handler intercepta boton atras para cerrar modales.
 * QL48: Verificador de versión APK (Android).
 */

import { type ReactNode } from 'react';
import { useInicializadorAuth } from '@app/hooks/varios-5/useInicializadorAuth';
import { useWebSocket } from '@app/hooks/varios-9/useWebSocket';
import { useNotificacionesNativas } from '@app/hooks/notificaciones/useNotificacionesNativas';
import { useBackHandler } from '@app/hooks/varios-2/useBackHandler';
import { useVerificadorVersion } from '@app/hooks/varios-9/useVerificadorVersion';

interface Props {
    children: ReactNode;
}

export const InicializadorAuth = ({ children }: Props): JSX.Element => {
    useInicializadorAuth();
    useWebSocket();
    useNotificacionesNativas();
    useBackHandler();
    useVerificadorVersion();
    return <>{children}</>;
};

export default InicializadorAuth;
