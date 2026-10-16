/*
 * [204A-1] Bootstrap del SPA usando el framework Glory en modo SPA.
 * Replica EXACTAMENTE el flujo del frontend WordPress legacy:
 *   1. Inyecta GLORY_CONTEXT y stub de wp-json antes de cargar nada.
 *   2. Inyecta __GLORY_ROUTES__ con todas las rutas del legacy.
 *   3. Registra los islands del proyecto (App/React/appIslands.tsx -> src/legacy/appIslands.tsx).
 *   4. Arranca Glory hydration que monta PageRenderer + AppProvider (LayoutPrincipal).
 *
 * NO se reescribe nada del legacy: imports apuntan al arbol copiado tal cual.
 */

/* IMPORTANTE: shims primero, antes de cualquier import legacy. */
import './bootstrap/gloryContextShim';
import './bootstrap/wpJsonStub';

import { ROUTES } from './bootstrap/routes';

/* Inyectar mapa de rutas para que Glory active modo SPA. */
(window as unknown as { __GLORY_ROUTES__?: unknown }).__GLORY_ROUTES__ = ROUTES;

/* Estilos base de Glory (variables, componentes, prosa). */
import './glory-core/index.css';

/* Estilos del legacy que LayoutPrincipal espera ya cargados. */
import './legacy/styles/variables.css';
/* [15A-1] Tokens heredados que el CSS portado referencia y nadie definia. */
import './legacy/styles/tokensHeredados.css';
import './legacy/styles/reset.css';
import './legacy/styles/tipografia.css';
import './legacy/styles/layout.css';
import './legacy/styles/componentes/panelFlotante.css';
/* [15A-27] Receta compartida de FILA OPERABLE: la opcion de una lista, la entrada
 * de un menu, el item de la barra lateral. Va aqui, con las demas hojas de receta
 * y ANTES de las hojas de componente: la receta aporta la piel y cada hoja se
 * queda su layout, asi que el orden natural es receta -> componente. Los dos
 * sitios donde la clase base de un control (.botonBase, .editarCheckbox) declara
 * la misma piel se resuelven dentro de la propia receta, nombrando el par. */
import './legacy/styles/componentes/filaOperable.css';

import { islandRegistry } from './glory-core/core';
import { initializeIslands } from './glory-core/core/hydration';
import appIslands, { AppProvider } from './legacy/appIslands';

islandRegistry.registerAll(appIslands);

function init(): void {
    initializeIslands({ appProvider: AppProvider });
}

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}

