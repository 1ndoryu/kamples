/*
 * ModalReportar — Kamples (QQ38)
 * Modal centralizado de reportes. Adapta UI segun tipo:
 * - error_plataforma: campo asunto + descripcion + URL auto
 * - usuario: muestra "@username" + motivo + detalles
 * - publicacion/comentario/sample: motivo + detalles
 * Reemplaza ModalReportarUsuario y ModalReportarError.
 */

import { Modal } from '../../../ui/modal/Modal';
import { BotonBase } from '../../../ui/boton/BotonBase';
import { Input } from '../../../ui/input/Input';
import { CampoTexto } from '../../../ui/varios-1/CampoTexto';
import { useReportar } from '@app/hooks/varios-7/useReportar';
import { useT } from '@app/utils/i18n/useT';
import '../../../../styles/componentes/modal/reportar/modalReportarError.css';
import '../../../../styles/componentes/modal/reportar/modalReportar.css';

export const ModalReportar = (): JSX.Element | null => {
    const {
        abierto,
        tipo,
        targetNombre,
        config,
        esError,
        razon,
        setRazon,
        detalles,
        setDetalles,
        enviando,
        puedeEnviar,
        enviar,
        cerrar,
    } = useReportar();

    const { t } = useT();

    if (!abierto || !tipo) return null;

    return (
        <Modal
            abierto={abierto}
            onCerrar={cerrar}
            tamano="pequeno"
        >
            <div className="modalFormulario reportarErrorFormulario">
                {tipo === 'usuario' && targetNombre && (
                    <p className="reportarErrorNota modalReportar-inl1">
                        {t('reporte.reportandoA')} <strong>@{targetNombre}</strong>
                    </p>
                )}

                <label className="reportarErrorLabel" htmlFor="reportarRazon">
                    {config.etiqueta}
                </label>
                <Input
                    id="reportarRazon"
                    className="reportarErrorInput"
                    type="text"
                    placeholder={config.placeholder}
                    value={razon}
                    onChange={(e) => setRazon(e.target.value)}
                    maxLength={500}
                    autoFocus
                />

                <label className="reportarErrorLabel" htmlFor="reportarDetalles">
                    {esError ? t('reporte.descripcion') : t('reporte.detallesAdicionales')}
                </label>
                <CampoTexto
                    multilínea
                    variante="desnudo"
                    id="reportarDetalles"
                    className="reportarErrorTextarea"
                    placeholder={
                        esError
                            ? 'Describe que paso, que esperabas que ocurriera y los pasos para reproducir el error...'
                            : 'Describe la situacion con mas detalle si lo deseas...'
                    }
                    value={detalles}
                    onChange={(e) => setDetalles(e.target.value)}
                    maxLength={2000}
                    rows={esError ? 5 : 4}
                />

                {esError && (
                    <p className="reportarErrorNota">
                        {t('reporte.seIncluyePagina')}
                    </p>
                )}

                <div className="modalAcciones">
                    <BotonBase
                        variante="secundario"
                        onClick={cerrar}
                        type="button"
                    >
                        {t('comun.cancelar')}
                    </BotonBase>
                    <BotonBase
                        variante="primario"
                        onClick={enviar}
                        disabled={!puedeEnviar}
                        type="button"
                    >
                        {enviando ? t('reporte.enviando') : t('reporte.enviarReporte')}
                    </BotonBase>
                </div>
            </div>
        </Modal>
    );
};

export default ModalReportar;
