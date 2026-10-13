/*
 * Barrel export — Services
 * Importar desde '@app/services' para acceso centralizado.
 */

export { crearLogger, configurarLogger, NivelLog } from './varios-1/logger';
export type { Logger } from './varios-1/logger';

export { apiPeticion, apiGet, apiPost, apiPut, apiDelete } from './api/varios-1/apiCliente';
export type { RespuestaApi } from './api/varios-1/apiCliente';

export { listarSamples, obtenerSample, obtenerFeed, subirSample } from './api/varios-3/apiSamples';
export type { FiltrosSamples, PaginacionSamples, RespuestaListaSamples } from './api/varios-3/apiSamples';

export { obtenerUsuarioActual, obtenerPerfil, actualizarPerfil, login, registrar } from './api/varios-1/apiAuth';

export { seguirUsuario, dejarDeSeguir, darLike, quitarLike, crearPublicacion, obtenerFeedInicio } from './api/varios-3/apiSocial';

export { obtenerConversaciones, obtenerMensajes, enviarMensaje, marcarConversacionLeida, iniciarConversacion } from './api/varios-2/apiMensajes';

export { obtenerEstadisticasCreador, obtenerTopSamples, obtenerTransacciones, obtenerIngresosPorPeriodo, solicitarPayout } from './api/varios-2/apiPagos';
export type { EstadisticasCreador, SampleStats, TransaccionCreador, IngresosPorPeriodo } from './api/varios-2/apiPagos';

export { obtenerNotificaciones, marcarLeida, marcarTodasLeidas } from './api/varios-2/apiNotificaciones';
export type { Notificacion, TipoNotificacion } from './api/varios-2/apiNotificaciones';

export { obtenerLimites, descargarSample, puedeDescargar } from './api/varios-2/apiDescargas';
export type { LimitesDescarga, ResultadoDescarga } from './api/varios-2/apiDescargas';

export { listarColecciones, crearColeccion, actualizarColeccion, eliminarColeccion } from './api/varios-2/apiColecciones';

export { wsService } from './varios-2/wsService';
export type { MensajeWS, EstadoConexion, HandlerMensaje } from './varios-2/wsService';
