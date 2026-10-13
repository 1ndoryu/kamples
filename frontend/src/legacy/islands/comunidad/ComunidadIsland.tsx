/*
 * Isla: ComunidadIsland — Kamples
 * Feed de posts sociales. Lógica extraída a useComunidadIsland (SRP).
 * Los posts se renderizan con TarjetaPublicacion — mismo componente que PerfilIsland.
 * Extras de isla: botón seguir (+) sobre el avatar, CardPerfil, sección comentarios.
 */

import { Users, TrendingUp, Clock, Plus, RefreshCw } from 'lucide-react';
import { TarjetaPublicacion } from '@app/components/social/tarjeta/TarjetaPublicacion';
import { MenuContextual } from '@app/components/ui/varios-2/MenuContextual';
import { ListaComentarios } from '@app/components/social/varios-2/ListaComentarios';
import { SeccionPublicar } from '@app/components/social/seccion/SeccionPublicar';
import { BotonBase } from '@app/components/ui/boton/BotonBase';
import { SkeletonTarjetaPublicacion } from '@app/components/skeletons';
import { EstadoVacio } from '@app/components/ui/varios-1/EstadoVacio';
import { useTabsIsla } from '@app/hooks/varios-8/useTabsIsla';
import { useAuthStore } from '@app/stores/auth/authStore';
import { LandingPublica } from '@app/components/social/varios-1/LandingPublica';
import { useComentarios } from '@app/hooks/varios-3/useComentarios';
import { useComunidadIsland, type FiltroComunidad } from '@app/hooks/varios-3/useComunidadIsland';
import { usePullToRefresh } from '@app/hooks/varios-6/usePullToRefresh';
import { useEsMovil } from '@app/hooks/varios-4/useEsMovil';
import { useT } from '@app/utils/i18n';
import { useTooltipPerfilStore } from '@app/stores/varios-4/tooltipPerfilStore';
import type { UsuarioResumen } from '@app/types/varios-1/usuario';
import '../../styles/componentes/comunidad/comunidad.css';
import '../../styles/componentes/comunidad/comunidadIsland.css';

/* [193A-65] Tabs se construyen dentro del componente para acceder a t() */

/* Sub-componente: comentarios por post */
const SeccionComentariosPost = ({ postId, navegar }: { postId: number; navegar: (ruta: string) => void }): JSX.Element => {
    const {
        comentarios, cargando, enviar, enviarMultimedia, cargarMas, hayMas,
        editar, eliminar, reportar, toggleLike, cargarRespuestas,
        editandoId, setEditandoId, respondendoAId, setRespondendoAId,
    } = useComentarios({ tipo: 'publicacion', targetId: postId, cargarAlAbrir: true });

    return (
        <ListaComentarios
            comentarios={comentarios} cargando={cargando} onEnviar={enviar} onEnviarMultimedia={enviarMultimedia}
            onClickAutor={(u) => navegar(`/perfil/${u}/`)} maxVisibles={3}
            onCargarMas={cargarMas} hayMasPaginas={hayMas}
            onEditar={editar} onEliminar={eliminar} onReportar={reportar} onToggleLike={toggleLike}
            onCargarRespuestas={cargarRespuestas} editandoId={editandoId} setEditandoId={setEditandoId}
            respondendoAId={respondendoAId} setRespondendoAId={setRespondendoAId}
        />
    );
};

/* [193A-65] Filtros se construyen dentro del componente para acceder a t() */

/* Contenido autenticado de la comunidad — hooks siempre se ejecutan */
const ComunidadContenido = (): JSX.Element => {
    const {
        publicaciones, filtro, setFiltro, cargando, cargandoMas, hayMas,
        comentariosAbiertos, navegar, usuario,
        menuSample, menuPublicacion, sentinelRef,
        recargarFeed, manejarLikePost, manejarLikeSample, manejarRepost, alternarComentarios,
    } = useComunidadIsland();

    const { t } = useT();

    /* [193A-65] Construir tabs y filtros con t() para i18n */
    const tabsComunidad = [{ id: 'comunidad', etiqueta: t('social.comunidad') }];
    const filtros: { valor: FiltroComunidad; icono: typeof Users; label: string }[] = [
        { valor: 'todos', icono: Clock, label: t('comun.todos') },
        { valor: 'siguiendo', icono: Users, label: t('comun.siguiendo') },
        { valor: 'populares', icono: TrendingUp, label: t('comun.populares') },
    ];

    useTabsIsla('ComunidadIsland', tabsComunidad, 'comunidad');

    /* [183A-38] Pull-to-refresh en comunidad para movil */
    const esMovil = useEsMovil();
    const pullToRefresh = usePullToRefresh({
        onRefrescar: recargarFeed,
        habilitado: esMovil,
    });

    /* QQ47: Abrir tooltip de perfil al hacer clic en el boton + sobre el avatar */
    const abrirCardPerfil = (e: React.MouseEvent, autor: UsuarioResumen) => {
        e.stopPropagation();
        const rect = e.currentTarget.getBoundingClientRect();
        useTooltipPerfilStore.getState().abrirInmediato(autor.username, {
            top: rect.top,
            left: rect.left,
            width: rect.width,
            height: rect.height,
        });
    };

    return (
        <div className="comunidadIsland" id="comunidadIsland">
            <SeccionPublicar alPublicar={recargarFeed} placeholder="¿Qué estás creando?" />

            <div className="comunidadBarraSuperior">
                <div className="comunidadFiltros">
                    {filtros.map(({ valor, icono: Icono, label }) => (
                        <BotonBase
                            variante="ghost"
                            key={valor}
                            className={`comunidadFiltroBtn ${filtro === valor ? 'comunidadFiltroBtnActivo' : ''}`}
                            onClick={() => setFiltro(valor)}
                            type="button"
                        >
                            <Icono size={14} /> {label}
                        </BotonBase>
                    ))}
                </div>
            </div>

            <div className="comunidadFeed" ref={pullToRefresh.contenedorRef}>
                {/* [183A-38] Indicador pull-to-refresh */}
                {esMovil && (pullToRefresh.distanciaArrastre > 0 || pullToRefresh.refrescando) && (
                    <div
                        className={`feedPullIndicador ${pullToRefresh.refrescando ? 'feedPullRefrescando' : ''}`}
                        style={{ height: pullToRefresh.distanciaArrastre }}
                    >
                        <RefreshCw
                            size={20}
                            className={pullToRefresh.refrescando ? 'feedPullGirando' : ''}
                            style={{ transform: `rotate(${pullToRefresh.distanciaArrastre * 3}deg)` }}
                        />
                    </div>
                )}
                {cargando ? (
                    <>
                        <SkeletonTarjetaPublicacion />
                        <SkeletonTarjetaPublicacion />
                        <SkeletonTarjetaPublicacion />
                    </>
                ) : publicaciones.length === 0 ? (
                    <EstadoVacio mensaje="No hay publicaciones aún" />
                ) : (
                    publicaciones.map((post) => (
                        <TarjetaPublicacion
                            key={post.id}
                            publicacion={post}
                            onLike={(id, reaccion) => manejarLikePost(id, reaccion)}
                            onComentar={(id) => alternarComentarios(id)}
                            onRepost={(id) => manejarRepost(id)}
                            onClickAutor={(username) => navegar(`/perfil/${username}/`)}
                            onClickFecha={(pubId) => navegar(`/publicacion/${pubId}/`)}
                            onMenu={(e, pub) => menuPublicacion.abrirMenu(e, pub)}
                            onLikeSample={manejarLikeSample}
                            onMenuSample={menuSample.abrirMenu}
                            onClickCreadorSample={(u) => navegar(`/perfil/${u}/`)}
                            mostrarCeroConteo
                            avatarExtra={
                                String(post.autor.id) !== String(usuario?.id) ? (
                                    <BotonBase
                                        variante="ghost"
                                        className="comunidadBtnSeguirIcono"
                                        onClick={(e) => abrirCardPerfil(e, post.autor)}
                                        aria-label="Ver perfil y seguir"
                                    >
                                        <Plus size={11} strokeWidth={2.5} />
                                    </BotonBase>
                                ) : undefined
                            }
                        >
                            {comentariosAbiertos.has(post.id) && (
                                <SeccionComentariosPost postId={post.id} navegar={navegar} />
                            )}
                        </TarjetaPublicacion>
                    ))
                )}

                {/* Sentinel para IntersectionObserver — scroll infinito */}
                {!cargando && hayMas && (
                    <div ref={sentinelRef} className="comunidadSentinel" aria-hidden="true" />
                )}
                {cargandoMas && (
                    <>
                        <SkeletonTarjetaPublicacion />
                        <SkeletonTarjetaPublicacion />
                    </>
                )}
            </div>

            <MenuContextual
                abierto={menuPublicacion.estado.abierto}
                onCerrar={menuPublicacion.cerrarMenu}
                items={menuPublicacion.items}
                x={menuPublicacion.estado.x}
                y={menuPublicacion.estado.y}
            />
            <MenuContextual
                abierto={menuSample.estado.abierto}
                onCerrar={menuSample.cerrarMenu}
                items={menuSample.items}
                x={menuSample.estado.x}
                y={menuSample.estado.y}
            />

            {/* TooltipPerfil ahora se renderiza globalmente en LayoutPrincipal */}
        </div>
    );
};

/*
 * Wrapper: muestra LandingPublica si no está autenticado,
 * ComunidadContenido si sí lo está. Esto evita que al cerrar el
 * modal de login quede la pantalla en negro (QQ82).
 */
const ComunidadBase = (): JSX.Element => {
    const autenticado = useAuthStore(s => s.autenticado);
    const cargandoAuth = useAuthStore(s => s.cargando);

    if (cargandoAuth) {
        return (
            <div className="comunidadIsland-inl1">
                Verificando sesión...
            </div>
        );
    }

    if (!autenticado) return <LandingPublica />;

    return <ComunidadContenido />;
};

export const ComunidadIsland = ComunidadBase;
export default ComunidadIsland;
