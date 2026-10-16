/*
 * Motor de hidratacion/montaje de islas React.
 * Extrae y mejora la logica que antes vivia en main.tsx.
 *
 * Soporta dos modos:
 *   A) Islas individuales: busca [data-island] y monta cada uno (legacy/fallback).
 *   B) SPA: si __GLORY_ROUTES__ existe, monta un PageRenderer unico
 *      que intercambia islas sin recarga al navegar.
 *
 * Flujo por isla (modo A):
 *   1. Busca contenedores [data-island] en el DOM
 *   2. Resuelve el componente via IslandRegistry
 *   3. Parsea props de data-props (JSON)
 *   4. Envuelve en: StrictMode > GloryProvider > AppProvider? > ErrorBoundary > Suspense? > DevOverlay?
 *   5. Monta con createRoot (CSR) o hydrateRoot (SSG)
 */

import { StrictMode, Suspense, type ComponentType, type ReactNode } from 'react';
import { createRoot, hydrateRoot } from 'react-dom/client';
import { islandRegistry } from './IslandRegistry';
import { IslandErrorBoundary } from './ErrorBoundary';
import { GloryProvider } from './GloryProvider';
import { DevOverlay } from './DevOverlay';
import { useNavigationStore } from './router/navigationStore';
import { PageRenderer } from './router/PageRenderer';
import type { GloryRoutesMap } from './router/navigationStore';
import { logger } from "../utils/logger";
import { rutaActual } from '../platform/ventana';
import { consultarElemento, consultarElementos } from '../platform/documento';

export interface InitOptions {
    appProvider?: ComponentType<{ children: ReactNode }>;
    suspenseFallback?: ReactNode;
}

const defaultSuspenseFallback = (
    <div className="gloryCargando">
        Cargando...
    </div>
);

/*
 * Construye el arbol de wrappers alrededor de un elemento.
 * AppProvider es opcional y definido por el proyecto usuario en appIslands.tsx.
 */
function wrapWithProviders(
    element: JSX.Element,
    appProvider?: ComponentType<{ children: ReactNode }>,
): JSX.Element {
    const AppProv = appProvider;
    const wrapped = AppProv ? <AppProv>{element}</AppProv> : element;
    return <GloryProvider>{wrapped}</GloryProvider>;
}

/*
 * Monta una isla individual en su contenedor DOM.
 */
function mountIsland(
    container: HTMLElement,
    islandName: string,
    props: Record<string, unknown>,
    options: InitOptions,
): void {
    const resolved = islandRegistry.resolve(islandName);
    if (!resolved) {
        logger.error(`[Glory] Componente "${islandName}" no registrado en IslandRegistry`);
        return;
    }

    const { component: Component, isLazy } = resolved;
    const shouldHydrate = container.dataset.hydrate === 'true';
    const hasContent =
        container.innerHTML.trim() !== '' &&
        !container.innerHTML.includes('<!-- react-island-loading -->');

    let islandContent: JSX.Element = <Component {...props} />;

    /* Lazy: envolver en Suspense para mostrar fallback mientras carga */
    if (isLazy) {
        islandContent = (
            <Suspense fallback={options.suspenseFallback ?? defaultSuspenseFallback}>
                {islandContent}
            </Suspense>
        );
    }

    /* DevOverlay solo en desarrollo */
    if (import.meta.env.DEV) {
        islandContent = (
            <DevOverlay islandName={islandName} props={props}>
                {islandContent}
            </DevOverlay>
        );
    }

    const element = (
        <StrictMode>
            {wrapWithProviders(
                <IslandErrorBoundary islandName={islandName}>
                    {islandContent}
                </IslandErrorBoundary>,
                options.appProvider,
            )}
        </StrictMode>
    );

    try {
        if (shouldHydrate && hasContent) {
            hydrateRoot(container, element);
            if (import.meta.env.DEV) {
                logger.warn(`[Glory] Isla "${islandName}" hidratada (SSG)`);
            }
        } else {
            container.innerHTML = '';
            createRoot(container).render(element);
            if (import.meta.env.DEV) {
                logger.warn(`[Glory] Isla "${islandName}" montada (CSR)`);
            }
        }
    } catch (error) {
        logger.error(`[Glory] Error montando isla "${islandName}":`, error);

        /* Fallback: si la hidratacion falla, intentar CSR */
        if (shouldHydrate) {
            logger.warn(`[Glory] Fallback a CSR para "${islandName}"`);
            try {
                container.innerHTML = '';
                createRoot(container).render(element);
            } catch (fallbackError) {
                logger.error(`[Glory] Fallback CSR tambien fallo para "${islandName}":`, fallbackError);
            }
        }
    }
}

/*
 * Inicializa todas las islas React encontradas en el DOM.
 * Si __GLORY_ROUTES__ esta disponible, activa modo SPA con PageRenderer.
 * Si no, monta islas individualmente (modo clasico).
 */
export function initializeIslands(options: InitOptions = {}): void {
    const routes = window.__GLORY_ROUTES__ as GloryRoutesMap | undefined;

    /* Modo SPA: hay rutas definidas, montar PageRenderer en el contenedor principal */
    if (routes && Object.keys(routes).length > 0) {
        initializeSPA(routes, options);
        return;
    }

    /* Modo clasico: montar islas individuales */
    initializeClassicIslands(options);
}

/*
 * Modo SPA: monta un unico PageRenderer que intercambia islas segun la ruta.
 * El contenedor [data-island] existente se reutiliza como root.
 */
function initializeSPA(routes: GloryRoutesMap, options: InitOptions): void {
    /*
     * Los callable props de PHP se omiten de __GLORY_ROUTES__ por diseño,
     * pero SÍ se evalúan y serializan en data-props del contenedor DOM.
     * Leer esos props evaluados y mergarlos en la ruta actual del mapa,
     * para que la isla inicial reciba datos como el usuario autenticado.
     */
    const contenedorInicial = consultarElemento<HTMLElement>('[data-island]');

    /* Extraer props evaluados del servidor (data-props del DOM).
     * Para rutas dinámicas (/cancion/slug), la búsqueda exacta en el mapa fallaría
     * porque el mapa solo tiene /cancion/. Se pasan directamente a inicializar,
     * que usa buscarRutaEnMapa (soporta prefijo) y los mergea sobre los props del mapa. */
    let propsEvaluadosServidor: Record<string, unknown> | undefined;
    if (contenedorInicial?.dataset.props) {
        try {
            propsEvaluadosServidor = JSON.parse(contenedorInicial.dataset.props) as Record<string, unknown>;
        } catch {
            /* JSON inválido en data-props: continuar sin props evaluados */
        }
    }

    /* Inicializar store con rutas + props evaluados del servidor para la ruta inicial */
    useNavigationStore.getState().inicializar(routes, rutaActual(), propsEvaluadosServidor);

    if (import.meta.env.DEV) {
        const rutasStr = Object.keys(routes).join(', ');
        logger.warn(`[Glory SPA] Modo SPA activo con ${Object.keys(routes).length} rutas: ${rutasStr}`);
    }

    /* Encontrar el contenedor principal [data-island] para montar el PageRenderer */
    const container = consultarElemento<HTMLElement>('[data-island]');
    if (!container) {
        if (import.meta.env.DEV) {
            logger.error('[Glory SPA] No se encontro contenedor [data-island] para PageRenderer');
        }
        return;
    }

    const AppProv = options.appProvider;

    const element = (
        <StrictMode>
            <GloryProvider>
                {AppProv ? (
                    <AppProv>
                        <PageRenderer suspenseFallback={options.suspenseFallback} />
                    </AppProv>
                ) : (
                    <PageRenderer suspenseFallback={options.suspenseFallback} />
                )}
            </GloryProvider>
        </StrictMode>
    );

    container.innerHTML = '';
    createRoot(container).render(element);

    if (import.meta.env.DEV) {
        const { islaActual } = useNavigationStore.getState();
        logger.warn(`[Glory SPA] PageRenderer montado, isla inicial: ${islaActual}`);

        /* [16A-4] Señal de montaje real para sondas automatizadas.
         * El aviso de arriba se escribe al *programar* el render, no al pintarlo,
         * y la app tarda varios segundos en tener contenido (el arranque espera
         * sondas de auth). Una sonda temprana leía `#root` vacío y lo tomaba por
         * un fallo. `window.__KAMPLES_MONTADO__` se pone cuando el contenedor ya
         * tiene elementos pintados: la sonda espera a esa bandera. */
        const marcarMontaje = (): void => {
            if (container.childElementCount > 0) {
                (window as unknown as { __KAMPLES_MONTADO__?: boolean }).__KAMPLES_MONTADO__ = true;
            } else {
                requestAnimationFrame(marcarMontaje);
            }
        };
        requestAnimationFrame(marcarMontaje);
    }
}

/*
 * Modo clasico: monta cada isla individualmente en su contenedor DOM.
 */
function initializeClassicIslands(options: InitOptions): void {
    const islands = consultarElementos<HTMLElement>('[data-island]');

    if (islands.length === 0) {
        if (import.meta.env.DEV) {
            logger.warn('[Glory] No se encontraron islas para montar');
        }
        return;
    }

    if (import.meta.env.DEV) {
        logger.warn(`[Glory] Montando ${islands.length} isla(s), registry: ${islandRegistry.getNames().join(', ')}`);
    }

    islands.forEach((container) => {
        const islandName = container.dataset.island;

        if (!islandName) {
            logger.error('[Glory] Contenedor sin nombre de isla:', container);
            return;
        }

        let props: Record<string, unknown> = {};
        const propsJson = container.dataset.props;

        if (propsJson) {
            try {
                props = JSON.parse(propsJson) as Record<string, unknown>;
            } catch (err) {
                logger.error(`[Glory] Error parseando props para "${islandName}":`, err);
            }
        }

        mountIsland(container, islandName, props, options);
    });
}
