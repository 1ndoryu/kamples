/*
 * VentanaSincPanel — Componente raiz de la ventana de sincronizacion.
 * Se renderiza como ventana Tauri independiente (sin Modal, sin layout).
 * Reutiliza usePanelSincronizacion y los sub-componentes de tabs existentes.
 *
 * La barra superior es draggable para mover la ventana frameless,
 * y el boton cerrar oculta la ventana (no la destruye).
 */

import { useCallback, useMemo, useState } from 'react';
import { EllipsisVertical, FolderOpen } from 'lucide-react';
import { BotonBase } from '@app/components/ui/boton/BotonBase';
import { ContenedorToasts } from '@app/components/ui/varios-1/ContenedorToasts';
import { usePanelSincronizacion } from '@app/hooks/panel/usePanelSincronizacion';
import '@app/styles/componentes/varios-6/sincronizacion.css';
import DiagnosticoSync from '../DiagnosticoSync';
import { ListaHistorialSync } from './ListaHistorialSync';
import { MenuPanelSync } from './MenuPanelSync';
import { iconoEstado } from './estadoVisual';
import { useAutoOcultarVentana } from './useAutoOcultarVentana';
import { usePerfilDesktop } from './usePerfilDesktop';
import { capitalizarPrimera, estadoLabel, repararMojibake } from './textoSync';

export function VentanaSincPanel(): JSX.Element {
    const {
        sincronizacionActiva,
        carpetaLocal,
        estado,
        mensajeEstado,
        historialSamples,
        elegirCarpeta,
        alternarSincronizacion,
        sincronizarAhora,
        reforzarSyncAhora,
        abrirCarpetaSincronizacion,
        limpiarHistorialLocal,
    } = usePanelSincronizacion();

    const [menu, setMenu] = useState<{ abierto: boolean; x: number; y: number }>({ abierto: false, x: 0, y: 0 });
    const [diagnosticoVisible, setDiagnosticoVisible] = useState(false);

    const ocultarVentana = useAutoOcultarVentana(setMenu);
    const perfilDesktop = usePerfilDesktop();

    const contexto = window.GLORY_CONTEXT as {
        currentUser?: { nombreVisible?: string; username?: string; avatarUrl?: string | null };
    } | undefined;

    const nombreUsuario = useMemo(() => {
        if (perfilDesktop?.nombre) return perfilDesktop.nombre;
        return contexto?.currentUser?.nombreVisible ?? contexto?.currentUser?.username ?? 'Usuario';
    }, [perfilDesktop?.nombre, contexto?.currentUser?.nombreVisible, contexto?.currentUser?.username]);

    const avatarUsuario = perfilDesktop?.avatarUrl ?? contexto?.currentUser?.avatarUrl ?? null;

    const estadoVisible = capitalizarPrimera(repararMojibake(mensajeEstado || estadoLabel(estado)));

    const abrirMenu = useCallback((e: React.MouseEvent<HTMLButtonElement>) => {
        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
        setMenu({ abierto: true, x: rect.right, y: rect.bottom + 6 });
    }, []);

    return (
        <>
            <div className="ventanaSincPanel ventanaSincPanelMinimal panelFlotante">
            <div className="sincPanelMinimalTop" data-tauri-drag-region>
                <div className="sincPanelPerfilBloque">
                    {avatarUsuario ? (
                        <img className="sincPanelPerfilAvatar" src={avatarUsuario} alt={nombreUsuario} />
                    ) : (
                        <div className="sincPanelPerfilAvatar sincPanelPerfilAvatarFallback">
                            {nombreUsuario.slice(0, 1).toUpperCase()}
                        </div>
                    )}
                    <span className="sincPanelPerfilNombre">{nombreUsuario}</span>
                </div>

                <div className="sincPanelMinimalDrag" />

                    <BotonBase
                        variante="ghost"
                        className="sincPanelMinimalMenu"
                        type="button"
                        aria-label="Opciones"
                        onClick={(e) => (menu.abierto ? setMenu(prev => ({ ...prev, abierto: false })) : abrirMenu(e))}
                    >
                        <EllipsisVertical size={14} />
                    </BotonBase>

                <MenuPanelSync
                    menu={menu}
                    onCerrar={() => setMenu(prev => ({ ...prev, abierto: false }))}
                    sincronizacionActiva={sincronizacionActiva}
                    diagnosticoVisible={diagnosticoVisible}
                    setDiagnosticoVisible={setDiagnosticoVisible}
                    ocultarVentana={ocultarVentana}
                    acciones={{
                        sincronizarAhora,
                        reforzarSyncAhora,
                        elegirCarpeta,
                        abrirCarpetaSincronizacion,
                        alternarSincronizacion,
                        limpiarHistorialLocal,
                    }}
                />
            </div>

            <ListaHistorialSync
                historialSamples={historialSamples}
                carpetaLocal={carpetaLocal}
                sincronizacionActiva={sincronizacionActiva}
                elegirCarpeta={elegirCarpeta}
                alternarSincronizacion={alternarSincronizacion}
                sincronizarAhora={sincronizarAhora}
            />

            {/* F6.2: Panel de diagnóstico colapsable */}
            {diagnosticoVisible && <DiagnosticoSync />}

            <div className={`sincPanelFooterMinimal sincPanelIndicador--${estado}`}>
                <div className="sincPanelFooterEstadoMinimal">
                    {iconoEstado(estado)}
                    <span>{estadoVisible}</span>
                </div>
                <BotonBase
                    variante="ghost"
                    className="sincPanelFooterCarpetaMinimal"
                    onClick={abrirCarpetaSincronizacion}
                    type="button"
                    aria-label="Abrir carpeta de sincronización"
                >
                    <FolderOpen size={15} />
                </BotonBase>
            </div>
            </div>
            <ContenedorToasts />
        </>
    );
}
