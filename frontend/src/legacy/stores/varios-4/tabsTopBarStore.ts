/*
 * Store: tabsTopBarStore — Kamples
 * Gestiona las tabs que se muestran en el TopBar.
 * Cada isla define sus tabs al montarse via setTabs.
 *
 * tabsPorIsla: guarda la última tab seleccionada por cada isla (keep-alive).
 * Cuando una isla vuelve a activarse, se restaura su tab anterior
 * en vez de resetear siempre a la inicial.
 *
 * [16A-4] tabsPorIsla se persiste en sessionStorage. Sin eso, la memoria
 * keep-alive moría con la pestaña y una recarga volvía siempre a la tab
 * inicial: el panel de admin nunca escribe `?tab=` en la URL (solo lo hacen
 * las islas con useUrlFiltros), así que no había de dónde restaurar.
 * Es sessionStorage y no localStorage a propósito: la tab abierta es estado
 * de la pestaña, no del navegador; con localStorage dos pestañas abiertas en
 * islas distintas se pisarían la memoria la una a la otra.
 */

import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';
import { busquedaActual } from '@/platform/ventana';

export interface TabTopBar {
    id: string;
    etiqueta: string;
    icono?: React.ReactNode;
}

interface EstadoTabsTopBar {
    tabs: TabTopBar[];
    activa: string;
    islaTabsActual: string | null;
    /* Memoria de última tab seleccionada por isla (persistencia keep-alive) */
    tabsPorIsla: Record<string, string>;
    setTabs: (tabs: TabTopBar[], activaInicial?: string, islaId?: string) => void;
    setActiva: (id: string) => void;
    /* Registra la tab activa para una isla en la memoria keep-alive */
    guardarTabIsla: (islaId: string, tabId: string) => void;
    limpiar: (islaId?: string) => void;
}

export const useTabsTopBarStore = create<EstadoTabsTopBar>()(
    persist(
        (set, get) => ({
            tabs: [],
            activa: '',
            islaTabsActual: null,
            tabsPorIsla: {},
            setTabs: (tabs, activaInicial, islaId) => {
                /* QK56: Prioridad de restauración: URL param > memoria keep-alive > inicial */
                const { tabsPorIsla } = get();
                let tabRestaurada = activaInicial ?? tabs[0]?.id ?? '';

                /* Restaurar desde URL si hay ?tab= */
                const urlTab = new URLSearchParams(busquedaActual()).get('tab');
                if (urlTab && tabs.some(t => t.id === urlTab)) {
                    tabRestaurada = urlTab;
                } else if (islaId && tabsPorIsla[islaId]) {
                    /* Verificar que la tab guardada sigue existiendo en las tabs actuales */
                    const existe = tabs.some(t => t.id === tabsPorIsla[islaId]);
                    if (existe) {
                        tabRestaurada = tabsPorIsla[islaId];
                    }
                }
                set({ tabs, activa: tabRestaurada, islaTabsActual: islaId ?? null });
            },
            setActiva: (activa) => set({ activa }),
            guardarTabIsla: (islaId, tabId) =>
                set(state => ({
                    tabsPorIsla: { ...state.tabsPorIsla, [islaId]: tabId },
                })),
            limpiar: (islaId) =>
                set(state => {
                    if (islaId && state.islaTabsActual !== islaId) {
                        return state;
                    }

                    return { ...state, tabs: [], activa: '', islaTabsActual: null };
                }),
        }),
        {
            name: 'kamples-tabs-topbar',
            storage: createJSONStorage(() => sessionStorage),
            /* Solo la memoria por isla. `tabs`, `activa` e `islaTabsActual`
             * describen lo que hay pintado ahora y se recalculan al montar:
             * persistirlos dejaría el TopBar con tabs de una isla que ya no
             * está activa. */
            partialize: (state) => ({ tabsPorIsla: state.tabsPorIsla }),
        },
    ),
);
