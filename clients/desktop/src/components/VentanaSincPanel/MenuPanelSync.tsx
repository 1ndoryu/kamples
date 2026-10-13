import { useMemo, type Dispatch, type SetStateAction } from 'react';
import {
    EyeOff,
    FolderOpen,
    FolderSync,
    PauseCircle,
    RefreshCw,
    Settings,
    Trash2,
} from 'lucide-react';
import { MenuContextual, type MenuItemDef } from '@app/components/ui/varios-2/MenuContextual';
import type { usePanelSincronizacion } from '@app/hooks/panel/usePanelSincronizacion';
import { abrirVentanaConfig } from './accionesVentana';

type PanelSync = ReturnType<typeof usePanelSincronizacion>;

interface MenuPanelSyncProps {
    menu: { abierto: boolean; x: number; y: number };
    onCerrar: () => void;
    sincronizacionActiva: boolean;
    diagnosticoVisible: boolean;
    setDiagnosticoVisible: Dispatch<SetStateAction<boolean>>;
    ocultarVentana: () => Promise<void>;
    acciones: Pick<
        PanelSync,
        | 'sincronizarAhora'
        | 'reforzarSyncAhora'
        | 'elegirCarpeta'
        | 'abrirCarpetaSincronizacion'
        | 'alternarSincronizacion'
        | 'limpiarHistorialLocal'
    >;
}

/* Menu contextual de opciones de la ventana de sincronizacion (boton ⋮ de la cabecera). */
export function MenuPanelSync({
    menu,
    onCerrar,
    sincronizacionActiva,
    diagnosticoVisible,
    setDiagnosticoVisible,
    ocultarVentana,
    acciones,
}: MenuPanelSyncProps): JSX.Element {
    const {
        sincronizarAhora,
        reforzarSyncAhora,
        elegirCarpeta,
        abrirCarpetaSincronizacion,
        alternarSincronizacion,
        limpiarHistorialLocal,
    } = acciones;

    const menuItems = useMemo<MenuItemDef[]>(() => [
        {
            id: 'sincronizar',
            etiqueta: 'Sincronizar ahora',
            icono: <FolderSync size={14} />,
            onClick: sincronizarAhora,
        },
        {
            id: 'reforzar-sync',
            etiqueta: 'Reforzar sincronización',
            icono: <RefreshCw size={14} />,
            onClick: reforzarSyncAhora,
        },
        {
            id: 'elegir-carpeta',
            etiqueta: 'Elegir carpeta',
            icono: <FolderOpen size={14} />,
            onClick: elegirCarpeta,
        },
        {
            id: 'abrir-carpeta',
            etiqueta: 'Abrir carpeta',
            icono: <FolderOpen size={14} />,
            onClick: abrirCarpetaSincronizacion,
        },
        {
            id: 'toggle-sync',
            etiqueta: sincronizacionActiva ? 'Pausar sync' : 'Activar sync',
            icono: <PauseCircle size={14} />,
            onClick: alternarSincronizacion,
        },
        {
            id: 'configuracion',
            etiqueta: 'Configuración',
            icono: <Settings size={14} />,
            onClick: () => { void abrirVentanaConfig(); },
        },
        {
            id: 'limpiar-historial',
            etiqueta: 'Limpiar historial',
            icono: <Trash2 size={14} />,
            peligro: true,
            onClick: limpiarHistorialLocal,
        },
        {
            id: 'diagnostico',
            etiqueta: diagnosticoVisible ? 'Ocultar diagnóstico' : 'Diagnóstico sync',
            icono: <Settings size={14} />,
            onClick: () => setDiagnosticoVisible(v => !v),
        },
        {
            id: 'ocultar',
            etiqueta: 'Ocultar panel',
            icono: <EyeOff size={14} />,
            onClick: () => { void ocultarVentana(); },
        },
    ], [
        abrirCarpetaSincronizacion,
        alternarSincronizacion,
        diagnosticoVisible,
        elegirCarpeta,
        limpiarHistorialLocal,
        ocultarVentana,
        reforzarSyncAhora,
        sincronizacionActiva,
        sincronizarAhora,
    ]);

    return (
        <MenuContextual
            abierto={menu.abierto}
            onCerrar={onCerrar}
            items={menuItems}
            x={menu.x}
            y={menu.y}
            alinearDerecha
            forzarDropdown
        />
    );
}
