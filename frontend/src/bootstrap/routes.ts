/*
 * [204A-1] Mapa de rutas GloryRoutesMap.
 * Replica EXACTAMENTE el mapeo URL -> Island del legacy WordPress (App/Config/pages.php).
 * Glory's PageRenderer lee este mapa via window.__GLORY_ROUTES__ para hacer SPA navigation.
 *
 * Reglas:
 * - Toda ruta termina en '/' (Glory normaliza asi).
 * - Para rutas dinamicas (perfil, sample, coleccion, etc.) se define el prefijo y
 *   opcionalmente un patron de params. Los segmentos extra de la URL se pasan como props.
 */

import type { GloryRoutesMap } from '../glory-core/core/router/navigationStore';

export const ROUTES: GloryRoutesMap = {
    /* InicioIsland muestra LandingPublica para deslogueados y el feed para autenticados.
     * Es la pagina raiz correcta. BienvenidaIsland es solo el onboarding post-registro. */
    '/': { island: 'InicioIsland', props: {}, title: 'Kamples' },
    '/home/': { island: 'InicioIsland', props: {}, title: 'Inicio - Kamples' },
    '/bienvenida/': { island: 'BienvenidaIsland', props: {}, title: 'Bienvenida - Kamples' },
    '/samples/': { island: 'FeedSamplesIsland', props: {}, title: 'Samples' },

    /* Auth: en SPA viven como rutas; los islands LoginIsland/RegistroIsland renderizan
     * los formularios completos. Tambien existe ModalAuth que se abre desde otras pantallas. */
    '/auth/login/': { island: 'LoginIsland', props: {}, title: 'Iniciar sesion' },
    '/auth/registro/': { island: 'RegistroIsland', props: {}, title: 'Crear cuenta' },

    /* Perfiles dinamicos: /perfil/:username */
    '/perfil/': { island: 'PerfilIsland', props: {}, title: 'Perfil', params: ':username' },
    '/perfil/editar/': { island: 'EditarPerfilIsland', props: {}, title: 'Editar perfil' },

    '/libreria/': { island: 'LibreriaIsland', props: {}, title: 'Libreria' },
    '/descargas/': { island: 'DescargasIsland', props: {}, title: 'Descargas' },
    '/favoritos/': { island: 'FavoritosIsland', props: {}, title: 'Favoritos' },
    '/reproductor/': { island: 'ReproductorIsland', props: {}, title: 'Reproductor' },
    '/descubrir/': { island: 'DescubrirIsland', props: {}, title: 'Descubrir' },
    '/colecciones/': { island: 'ColeccionesIsland', props: {}, title: 'Colecciones' },
    /* Sin ruta /planes/: los planes se abren con el modal de `planesModalStore`, que sustituyo a
     * la navegacion (ver el comentario del store). `PlanesIsland` sigue montada por
     * `LayoutPrincipal`, pero como isla de ruta devolvia `<></>` — `abierto` nace en false — y la
     * pagina quedaba en blanco sin decir nada. La landing de precios es `/precios/`. */
    '/precios/': { island: 'PreciosLandingIsland', props: {}, title: 'Precios' },
    '/comunidad/': { island: 'ComunidadIsland', props: {}, title: 'Comunidad' },

    /* Publicaciones dinamicas: /publicacion/:publicacionId y /post/:publicacionId */
    '/publicacion/': { island: 'PublicacionIsland', props: {}, title: 'Publicacion', params: ':publicacionId' },
    '/post/': { island: 'PublicacionIsland', props: {}, title: 'Publicacion', params: ':publicacionId' },

    /* Coleccion dinamica: /coleccion/:coleccionSlug */
    '/coleccion/': { island: 'ColeccionDetalleIsland', props: {}, title: 'Coleccion', params: ':coleccionSlug' },

    '/mensajes/': { island: 'MensajesIsland', props: {}, title: 'Mensajes' },
    /* Chat individual: /mensajes/chat/:conversacionId. Sin `params` la ruta no recibia el id
     * y el chat no abria; `/mensajes/<id>/` tampoco vale: el router lo resuelve por prefijo
     * a `/mensajes/` y vuelve a pintar la lista. */
    '/mensajes/chat/': { island: 'ChatIsland', props: { conversacionId: '' }, title: 'Chat', params: ':conversacionId' },

    '/admin/dashboard/': { island: 'DashboardCreadorIsland', props: {}, title: 'Dashboard' },
    '/admin/panel/': { island: 'AdminPanelIsland', props: {}, title: 'Admin' },

    /* Sample dinamico: /sample/:slug */
    '/sample/': { island: 'SampleDetalleIsland', props: {}, title: 'Sample' },

    '/musica/': { island: 'ExplorarCancionesIsland', props: {}, title: 'Musica' },

    /* Cancion dinamica: /cancion/:slug */
    '/cancion/': { island: 'CancionDetalleIsland', props: {}, title: 'Cancion' },

    /* Sampleo dinamico: /sampleo/:id/:slug? */
    '/sampleo/': { island: 'RelacionDetalleIsland', props: {}, title: 'Sampleo', params: ':id/:slug?' },

    /* Artista dinamico: /artista/:slug */
    '/artista/': { island: 'ArtistaDetalleIsland', props: {}, title: 'Artista', params: ':slug' },

    '/componentes/': { island: 'ShowcaseIsland', props: {}, title: 'Componentes' },
    '/dev/componentes/': { island: 'ShowcaseIsland', props: {}, title: 'Componentes' },

    /* [254A-B] Ruta /blog/ removida de produccion: el blog/articulos ya no es accesible. */

    '/privacy/': { island: 'PrivacidadIsland', props: {}, title: 'Privacidad' },
    '/terms/': { island: 'TerminosIsland', props: {}, title: 'Terminos' },
};
