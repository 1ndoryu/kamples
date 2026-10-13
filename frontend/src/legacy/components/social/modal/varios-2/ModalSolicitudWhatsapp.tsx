/*
 * ModalSolicitudWhatsapp — Kamples (QQ63)
 * Modal de solicitud de ingreso al grupo de WhatsApp del proyecto.
 * Temporal durante la beta. Reutiliza estilos de reportarError.
 * Restricciones: 1 por usuario, 6 por día globales.
 */

import { Modal } from '../../../ui/modal/Modal';
import { BotonBase } from '../../../ui/boton/BotonBase';
import { Input } from '../../../ui/input/Input';
import { CampoTexto } from '../../../ui/varios-1/CampoTexto';
import { useSolicitudWhatsapp } from '@app/hooks/varios-8/useSolicitudWhatsapp';
import { useT } from '@app/utils/i18n/useT';
import '../../../../styles/componentes/modal/reportar/modalReportarError.css';
import '../../../../styles/componentes/modal/varios-3/modalSolicitudWhatsapp.css';

export const ModalSolicitudWhatsapp = (): JSX.Element | null => {
    const {
        abierto,
        cargando,
        estado,
        nombre,
        setNombre,
        telefono,
        setTelefono,
        pais,
        setPais,
        motivo,
        setMotivo,
        descripcion,
        setDescripcion,
        enviando,
        puedeEnviar,
        enviar,
        cerrar,
    } = useSolicitudWhatsapp();

    /* [193A-64] useT DEBE ir antes de cualquier return condicional — Rules of Hooks */
    const { t } = useT();

    if (!abierto) return null;

    /* Mensaje de limite diario alcanzado */
    if (!cargando && estado.limiteDiario) {
        return (
            <Modal abierto={abierto} onCerrar={cerrar} tamano="pequeno">
                <div className="reportarErrorFormulario">
                    <p className="reportarErrorNota modalSolicitudWhatsapp-inl1">
                        {t('whatsapp.limiteDiario')}
                    </p>
                    <div className="modalAcciones">
                        <BotonBase variante="secundario" onClick={cerrar} type="button">{t('comun.cerrar')}</BotonBase>
                    </div>
                </div>
            </Modal>
        );
    }

    /* Mensaje de solicitud ya enviada */
    if (!cargando && estado.yaEnviada) {
        return (
            <Modal abierto={abierto} onCerrar={cerrar} tamano="pequeno">
                <div className="reportarErrorFormulario">
                    <p className="reportarErrorNota modalSolicitudWhatsapp-inl2">
                        {t('whatsapp.yaEnviada')}
                    </p>
                    <div className="modalAcciones">
                        <BotonBase variante="secundario" onClick={cerrar} type="button">{t('comun.cerrar')}</BotonBase>
                    </div>
                </div>
            </Modal>
        );
    }

    return (
        <Modal abierto={abierto} onCerrar={cerrar} tamano="pequeno">
            <div className="reportarErrorFormulario">
                <p className="reportarErrorNota modalSolicitudWhatsapp-inl3">
                    {t('whatsapp.descripcionBeta')}
                </p>

                <label className="reportarErrorLabel" htmlFor="solicitudNombre">{t('whatsapp.nombre')}</label>
                <Input
                    id="solicitudNombre"
                    className="reportarErrorInput"
                    type="text"
                    placeholder={t('whatsapp.nombrePlaceholder')}
                    value={nombre}
                    onChange={(e) => setNombre(e.target.value)}
                    maxLength={100}
                    autoFocus
                />

                <label className="reportarErrorLabel" htmlFor="solicitudTelefono">{t('whatsapp.telefono')}</label>
                <Input
                    id="solicitudTelefono"
                    className="reportarErrorInput"
                    type="tel"
                    placeholder={t('whatsapp.telefonoPlaceholder')}
                    value={telefono}
                    onChange={(e) => setTelefono(e.target.value)}
                    maxLength={20}
                />

                <label className="reportarErrorLabel" htmlFor="solicitudPais">{t('whatsapp.pais')}</label>
                <Input
                    id="solicitudPais"
                    className="reportarErrorInput"
                    type="text"
                    placeholder={t('whatsapp.paisPlaceholder')}
                    value={pais}
                    onChange={(e) => setPais(e.target.value)}
                    maxLength={60}
                />

                <label className="reportarErrorLabel" htmlFor="solicitudMotivo">{t('whatsapp.motivo')}</label>
                <Input
                    id="solicitudMotivo"
                    className="reportarErrorInput"
                    type="text"
                    placeholder={t('whatsapp.motivoPlaceholder')}
                    value={motivo}
                    onChange={(e) => setMotivo(e.target.value)}
                    maxLength={500}
                />

                <label className="reportarErrorLabel" htmlFor="solicitudDescripcion">{t('whatsapp.descripcionLabel')}</label>
                <CampoTexto
                    multilínea
                    variante="desnudo"
                    id="solicitudDescripcion"
                    className="reportarErrorTextarea"
                    placeholder={t('whatsapp.descripcionPlaceholder')}
                    value={descripcion}
                    onChange={(e) => setDescripcion(e.target.value)}
                    maxLength={2000}
                    rows={4}
                />

                <div className="modalAcciones">
                    <BotonBase variante="secundario" onClick={cerrar} type="button">
                        {t('comun.cancelar')}
                    </BotonBase>
                    <BotonBase
                        variante="primario"
                        onClick={enviar}
                        disabled={!puedeEnviar}
                        type="button"
                    >
                        {enviando ? t('whatsapp.enviando') : t('whatsapp.enviarSolicitud')}
                    </BotonBase>
                </div>
            </div>
        </Modal>
    );
};

export default ModalSolicitudWhatsapp;
