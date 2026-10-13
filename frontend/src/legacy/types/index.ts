/*
 * Barrel export de todos los tipos de Kamples.
 * Importar desde '@app/types' para acceso centralizado.
 */

export type {
    Sample,
    SampleResumen,
    MetadataSample,
    EstadoSample,
    TipoSample,
    TipoReaccion,
    NotaMusical,
    EscalaMusical,
    ExtraccionSample,
    ScoreDebug,
} from './varios-1/sample';

export type {
    Usuario,
    UsuarioResumen,
    UsuarioAutenticado,
    DatosSuspension,
    TipoPlan,
    RolUsuario,
} from './varios-1/usuario';

export type {
    Publicacion,
    Comentario,
    ComentarioDestacado,
    TipoPublicacion,
    RepostOriginal,
} from './varios-1/publicacion';

export type {
    Conversacion,
    Mensaje,
    TipoMensaje,
    MediaMetadata,
    MediaMetadataImagen,
    MediaMetadataAudio,
    MediaMetadataSample,
} from './varios-1/mensaje';

export type {
    Notificacion,
    TipoNotificacion,
} from './varios-1/notificacion';

export type {
    Plan,
    Suscripcion,
    Transaccion,
    NombrePlan,
    EstadoSuscripcion,
    TipoTransaccion,
    EstadoTransaccion,
} from './varios-1/plan';

export type {
    Coleccion,
    ColeccionResumen,
} from './varios-1/coleccion';

export type {
    Articulo,
    ArticuloResumen,
    CategoriaArticulo,
    ModeracionEstadoArticulo,
    EmbedArticulo,
    AdjuntoArticulo,
} from './varios-1/articulo';

export type {
    Cancion,
    CancionResumen,
    ArtistaMusicale,
    RelacionSample,
    CancionArtista,
    CancionDetalle,
    ArtistaDetalle,
    EstadisticaRelaciones,
    SeccionMusica,
    TipoRelacion,
    TipoElemento,
    FuenteRelacion,
    RolCancionArtista,
} from './varios-1/cancion';

export {
    ETIQUETAS_TIPO_RELACION,
    ETIQUETAS_TIPO_ELEMENTO,
    ETIQUETAS_ROL,
} from './varios-1/cancion';

/* Schema generado — interfaces DB crudas y constantes de columna */
export type {
    IAlgoritmoEstado,
    IColecciones,
    IColeccionSamples,
    IComentarios,
    IConversaciones,
    IDescargas,
    IFollows,
    ILikes,
    IMensajes,
    INotificaciones,
    IPublicaciones,
    IReportesDuplicados,
    IReportes,
    IReproducciones,
    ISamples,
    ISuscripciones,
    ITransacciones,
    IUsuariosExt,
} from './_generated/schema';

export {
    SamplesCols,
    UsuariosExtCols,
    ColeccionesCols,
    ColeccionSamplesCols,
    ComentariosCols,
    ConversacionesCols,
    DescargasCols,
    FollowsCols,
    LikesCols,
    MensajesCols,
    NotificacionesCols,
    PublicacionesCols,
    ReportesDuplicadosCols,
    ReportesCols,
    ReproduccionesCols,
    SuscripcionesCols,
    TransaccionesCols,
    AlgoritmoEstadoCols,
} from './_generated/schema';
