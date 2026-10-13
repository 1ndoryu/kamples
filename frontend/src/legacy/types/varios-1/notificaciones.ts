import type { Notificacion, TipoNotificacion } from '@app/services/api/varios-2/apiNotificaciones';

export type CategoriaNotificacion = 'social' | 'mensajes' | 'sistema' | 'monetizacion' | 'logro';
export type PrioridadNotificacion = 'baja' | 'media' | 'alta' | 'critica';

export interface DefinicionTipoNotificacion {
    categoria: CategoriaNotificacion;
    prioridad: PrioridadNotificacion;
    permiteToast: boolean;
}

export interface NotificacionUI extends Notificacion {
    categoria: CategoriaNotificacion;
    prioridad: PrioridadNotificacion;
    permiteToast: boolean;
    toastMensaje: string;
}

export type MapaTiposNotificacion = Record<TipoNotificacion, DefinicionTipoNotificacion>;