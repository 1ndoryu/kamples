/*
 * Componente: VisorImagen — Kamples (QQ52)
 * Modal overlay para ver imágenes a tamaño completo.
 * Usa el componente Modal del sistema para consistencia.
 */

import { useVisorImagenStore } from '@app/stores/varios-4/visorImagenStore';
import { Modal } from '../modal/Modal';
import '../../../styles/componentes/varios-7/visorImagen.css';

export const VisorImagen = (): JSX.Element | null => {
    const url = useVisorImagenStore(s => s.url);
    const alt = useVisorImagenStore(s => s.alt);
    const cerrar = useVisorImagenStore(s => s.cerrar);

    return (
        <Modal abierto={!!url} onCerrar={cerrar} className="visorImagenModal">
            {url && (
                <img
                    src={url}
                    alt={alt}
                    className="visorImagenImg"
                />
            )}
        </Modal>
    );
};

export default VisorImagen;
