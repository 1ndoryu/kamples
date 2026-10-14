/*
 * Componente: ModalConfiguracion  Kamples
 * Desktop: modal con panel lateral de navegacion.
 * Movil (QL89): pantalla completa con drill-down, sin wrapper Modal.
 * Logica en useModalConfiguracion (SRP), secciones en ConfiguracionSecciones.
 */

import {createPortal} from 'react-dom';
import {Save, ArrowLeft, X} from 'lucide-react';
import {BotonBase} from '@app/components/ui/boton/BotonBase';
import {Modal} from '@app/components/ui/modal/Modal';
import {useModalConfiguracion} from '@app/hooks/modal/varios-1/useModalConfiguracion';
import {useEsMovil} from '@app/hooks/varios-4/useEsMovil';
import {useRegistrarCapa} from '@app/hooks/varios-7/useRegistrarCapa';
import {useT} from '@app/utils/i18n/useT';
import {useVersionStore} from '@app/stores/varios-4/versionStore';
import {ContenidoSeccion, NavSecciones, SECCIONES_NAV, type HookConfiguracion} from '../../varios-1/ConfiguracionSecciones';
import '../../../../styles/componentes/modal/varios-1/modalConfiguracion.css';
import { cuerpoDocumento } from '@/platform/documento';

/* [TareaFinal-B] Pie de versión en el footer del nav lateral de configuraciones.
 * Muestra la versión del binario para la plataforma detectada (windows/apk).
 * En web muestra la versión web si está disponible, si no, nada. */
const VersionFooterConfig = (): JSX.Element | null => {
    const { t } = useT();
    const plataforma = useVersionStore(s => s.plataformaActual);
    const versions = useVersionStore(s => s.versions);
    const version = versions[plataforma];
    if (!version?.version) return null;
    return (
        <div className="configNavVersion">
            <span className="configNavVersionTexto">{t('config.version')} {version.version}</span>
        </div>
    );
};

/* Desktop: modal clasico con panel lateral */
const ConfiguracionDesktop = (h: HookConfiguracion): JSX.Element => {
    const { t } = useT();
    return (
    <Modal abierto={h.abierto && h.autenticado} onCerrar={h.manejarCerrar} className="configModalLayout">
        <div className="configNavLateral">
            <h3 className="configNavTitulo">{t('config.titulo')}</h3>
            <NavSecciones h={h} />
            <VersionFooterConfig />
        </div>
        <div className="configContenido">
            <div className="configSeccionContenido"><ContenidoSeccion h={h} /></div>
            <div className="configAcciones">
                <BotonBase variante="ghost" onClick={h.manejarCerrar} disabled={h.guardando}>{t('comun.cancelar')}</BotonBase>
                <BotonBase variante="primario" onClick={h.manejarGuardar} disabled={h.guardando}>
                    <Save size={14} /> {h.guardando ? t('config.guardando') : t('comun.guardar')}
                </BotonBase>
            </div>
        </div>
    </Modal>
    );
};

/* QL89+QL101: Movil  bottom sheet parcial tipo dropdown/menu contextual */
const ConfiguracionMovil = (h: HookConfiguracion): JSX.Element => {
    const { t } = useT();
    return createPortal(
    // sentinel-disable-next-line componente-artesanal — bottom sheet móvil (QL89/QL101) con overlay propio; <Modal> cambiaría el layout
    <div className="configMovilOverlay" onClick={h.manejarCerrar}>
        <div className="configMovilPantalla" onClick={(e) => e.stopPropagation()}>
            {h.movilEnMenu ? (
                <div className="configMovilNav">
                    <div className="configMovilCabecera">
                        <h3 className="configNavTitulo">{t('config.titulo')}</h3>
                        <BotonBase variante="ghost" className="configMovilCerrar" onClick={h.manejarCerrar} type="button" aria-label={t('comun.cerrar')}>
                            <X size={20} />
                        </BotonBase>
                    </div>
                    <NavSecciones h={h} />
                    <VersionFooterConfig />
                </div>
            ) : (
                <div className="configMovilDetalle">
                    <div className="configMovilCabecera">
                        <BotonBase variante="ghost" className="configMovilVolver" onClick={h.volverAlMenuMovil} type="button">
                            <ArrowLeft size={18} />
                            <span>{SECCIONES_NAV.find(s => s.id === h.seccionActiva)?.etiqueta ?? t('config.titulo')}</span>
                        </BotonBase>
                        <BotonBase variante="ghost" className="configMovilCerrar" onClick={h.manejarCerrar} type="button" aria-label={t('comun.cerrar')}>
                            <X size={20} />
                        </BotonBase>
                    </div>
                    <div className="configMovilContenido"><ContenidoSeccion h={h} /></div>
                    <div className="configAcciones">
                        <BotonBase variante="ghost" onClick={h.manejarCerrar} disabled={h.guardando}>{t('comun.cancelar')}</BotonBase>
                        <BotonBase variante="primario" onClick={h.manejarGuardar} disabled={h.guardando}>
                            <Save size={14} /> {h.guardando ? t('config.guardando') : t('comun.guardar')}
                        </BotonBase>
                    </div>
                </div>
            )}
        </div>
    </div>,
    cuerpoDocumento()
    );
};

export const ModalConfiguracion = (): JSX.Element | null => {
    const hookData = useModalConfiguracion();
    const {abierto, autenticado, manejarCerrar} = hookData;
    const esMovil = useEsMovil();

    useRegistrarCapa('configMovil', abierto && autenticado && esMovil, manejarCerrar);

    if (!abierto || !autenticado) return null;

    if (esMovil) return <ConfiguracionMovil {...hookData} />;
    return <ConfiguracionDesktop {...hookData} />;
};