/*
 * [Tarea Final] ModalVersionDesactualizada — Notifica al usuario cuando tiene
 * una versión desactualizada de Kamples (desktop o APK).
 *
 * Reaparece en cada recarga mientras el usuario no actualice.
 * El dismiss es solo en-memoria (no persiste): el modal vuelve a aparecer
 * en el próximo reload hasta que el usuario actualice.
 *
 * Solo se muestra en plataformas nativas (windows/apk).
 * En web la versión siempre está al día por definición.
 */

import { ArrowUpCircle } from 'lucide-react';
import { Modal } from './Modal';
import { CabeceraModal } from './CabeceraModal';
import { ModalAcciones } from './ModalAcciones';
import { BotonBase } from '../boton/BotonBase';
import { useVersionStore, type PlataformaApp } from '@app/stores/varios-4/versionStore';
import { useT } from '@app/utils/i18n';
import '../../../styles/componentes/modal/varios-3/modalVersionDesactualizada.css';

const NOMBRE_PLATAFORMA: Record<PlataformaApp, string> = {
    windows: 'Windows',
    apk: 'Android',
    web: 'Web',
};

export const ModalVersionDesactualizada = (): JSX.Element | null => {
    const { t } = useT();
    const modalAbierto = useVersionStore(s => s.modalAbierto);
    const cerrarModal = useVersionStore(s => s.cerrarModal);
    const versions = useVersionStore(s => s.versions);
    const plataformaActual = useVersionStore(s => s.plataformaActual);

    if (!modalAbierto || plataformaActual === 'web') return null;

    const infoLatest = versions[plataformaActual];
    if (!infoLatest?.version) return null;

    const versionActual = (window as unknown as Record<string, string>).__KAMPLES_VERSION__ ?? '0.0.0';
    const nombrePlataforma = NOMBRE_PLATAFORMA[plataformaActual];

    const manejarActualizar = () => {
        if (infoLatest.url) {
            window.open(infoLatest.url, '_blank', 'noopener');
        }
        cerrarModal();
    };

    return (
        <Modal
            abierto={modalAbierto}
            onCerrar={cerrarModal}
            tamano="pequeno"
        >
            <div className="modalVersion">
                <CabeceraModal
                    variante="centrada"
                    icono={<ArrowUpCircle size={24} />}
                    titulo={t('modal.actualizacion.titulo')}
                />

                <div className="modalVersionVersiones">
                    <div className="modalVersionFila">
                        <span className="modalVersionLabel">{t('modal.actualizacion.plataforma')}</span>
                        <span className="modalVersionValor">{nombrePlataforma}</span>
                    </div>
                    <div className="modalVersionFila">
                        <span className="modalVersionLabel">{t('modal.actualizacion.versionActual')}</span>
                        <span className="modalVersionValor">v{versionActual}</span>
                    </div>
                    <div className="modalVersionFila">
                        <span className="modalVersionLabel">{t('modal.actualizacion.versionNueva')}</span>
                        <span className="modalVersionValor modalVersionValorNuevo">v{infoLatest.version}</span>
                    </div>
                </div>

                {infoLatest.notes && (
                    <p className="modalVersionNotas">{infoLatest.notes}</p>
                )}

                <ModalAcciones>
                    <BotonBase variante="ghost" tamano="md" onClick={cerrarModal}>
                        {t('modal.actualizacion.omitir')}
                    </BotonBase>
                    {infoLatest.url && (
                        <BotonBase variante="primario" tamano="md" onClick={manejarActualizar}>
                            {t('modal.actualizacion.actualizar')}
                        </BotonBase>
                    )}
                </ModalAcciones>
            </div>
        </Modal>
    );
};
