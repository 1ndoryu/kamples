/*
 * Componente: CabeceraModal
 * Héroe de cabecera de modal: icono + título + subtítulo opcional.
 * Variantes: fila (icono al lado del título, alineado a la izquierda),
 *            centrada (icono destacado encima del título, todo centrado).
 * Tono del icono: acento (informativo), error (destructivo), neutro.
 *
 * [16A-4, por que] Ocho modales reimplementaban esta misma cabecera con ocho clases y
 * valores que ya habían divergido (18/22/24px y 500/600/700 para el mismo
 * papel). Una sola pieza con el sistema de tokens como fuente de verdad.
 */

import { type ReactNode } from 'react';
import '../../../styles/componentes/modal/cabeceraModal.css';

type VarianteCabecera = 'fila' | 'centrada';
type TonoIcono = 'acento' | 'error' | 'neutro';

interface CabeceraModalProps {
    titulo: ReactNode;
    icono?: ReactNode;
    subtitulo?: ReactNode;
    variante?: VarianteCabecera;
    tonoIcono?: TonoIcono;
    /* El título grande es para cabeceras a pantalla completa (onboarding). */
    tamanoTitulo?: 'normal' | 'grande';
}

const mapaTono: Record<TonoIcono, string> = {
    acento: '',
    error: 'cabeceraModalIcono--error',
    neutro: 'cabeceraModalIcono--neutro',
};

export const CabeceraModal = ({
    titulo,
    icono,
    subtitulo,
    variante = 'fila',
    tonoIcono = 'acento',
    tamanoTitulo = 'normal',
}: CabeceraModalProps): JSX.Element => {
    const clasesIcono = ['cabeceraModalIcono', mapaTono[tonoIcono]].filter(Boolean).join(' ');
    const clasesTitulo = ['cabeceraModalTitulo', tamanoTitulo === 'grande' ? 'cabeceraModalTitulo--grande' : '']
        .filter(Boolean)
        .join(' ');

    return (
        <div className={`cabeceraModal cabeceraModal--${variante}`}>
            {icono && <div className={clasesIcono}>{icono}</div>}
            <h3 className={clasesTitulo}>{titulo}</h3>
            {subtitulo && <p className="cabeceraModalSubtitulo">{subtitulo}</p>}
        </div>
    );
};

export default CabeceraModal;
