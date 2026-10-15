/*
 * ModalCombinarColeccion — Modal para combinar dos colecciones en una.
 * QL115: Selección de colección a fusionar, nombre/imagen a conservar.
 * QL120: Manejo de subcolecciones (mover o aplanar).
 */

import { Combine } from 'lucide-react';
import { Modal } from '@app/components/ui/modal/Modal';
import { BotonBase } from '@app/components/ui/boton/BotonBase';
import { ModalAcciones } from '@app/components/ui/modal/ModalAcciones';
import { SelectorMenu } from '@app/components/ui/selector/SelectorMenu';
import { useModalCombinarColeccion } from '@app/hooks/modal/varios-1/useModalCombinarColeccion';
import type { Coleccion } from '@app/types';
import '../../../../styles/componentes/modal/varios-1/modalCombinarColeccion.css';

interface ModalCombinarColeccionProps {
    abierto: boolean;
    onCerrar: () => void;
    onCombinado?: (destinoId: number) => void;
    coleccion: Coleccion | null;
    esAdmin?: boolean;
}

export const ModalCombinarColeccion = ({
    abierto,
    onCerrar,
    onCombinado,
    coleccion,
    esAdmin = false,
}: ModalCombinarColeccionProps): JSX.Element | null => {
    const {
        origenId,
        setOrigenId,
        nombreFuente,
        setNombreFuente,
        imagenFuente,
        setImagenFuente,
        manejoHijas,
        setManejoHijas,
        coleccionOrigen,
        opcionesCombinar,
        opcionesNombre,
        opcionesImagen,
        opcionesManejoHijas,
        origenTieneHijas,
        combinando,
        cargando,
        manejarCombinar,
    } = useModalCombinarColeccion({ abierto, onCerrar, onCombinado, coleccion, esAdmin });

    return (
        <Modal abierto={abierto} onCerrar={onCerrar} tamano="pequeno">
            <div className="modalCombinarContenido">
                <div className="modalCombinarIcono">
                    <Combine size={32} />
                </div>

                <h3 className="modalCombinarTitulo">Combinar colecciones</h3>
                <p className="modalCombinarDescripcion">
                    Fusiona otra colección con <strong>{coleccion?.nombre}</strong>.
                    Los samples se unirán y la colección seleccionada desaparecerá.
                </p>

                {cargando ? (
                    <div className="modalCombinarCargando">Cargando colecciones...</div>
                ) : (
                    <>
                        <div className="modalCampo modalCombinarCampo">
                            <span className="modalCombinarCampoEtiqueta">Combinar con</span>
                            <SelectorMenu
                                opciones={opcionesCombinar}
                                valor={origenId ? String(origenId) : ''}
                                onChange={setOrigenId}
                                placeholder="Seleccionar colección..."
                            />
                        </div>

                        {coleccionOrigen && (
                            <>
                                <div className="modalCampo modalCombinarCampo">
                                    <span className="modalCombinarCampoEtiqueta">Nombre a conservar</span>
                                    <SelectorMenu
                                        opciones={opcionesNombre}
                                        valor={nombreFuente}
                                        onChange={setNombreFuente}
                                    />
                                </div>

                                {opcionesImagen.length > 1 && (
                                    <div className="modalCampo modalCombinarCampo">
                                        <span className="modalCombinarCampoEtiqueta">Imagen de portada</span>
                                        <SelectorMenu
                                            opciones={opcionesImagen}
                                            valor={imagenFuente}
                                            onChange={setImagenFuente}
                                        />
                                    </div>
                                )}

                                {/* QL120: Si la colección origen tiene subcolecciones */}
                                {origenTieneHijas && (
                                    <div className="modalCampo modalCombinarCampo">
                                        <span className="modalCombinarCampoEtiqueta">Subcolecciones</span>
                                        <SelectorMenu
                                            opciones={opcionesManejoHijas}
                                            valor={manejoHijas}
                                            onChange={setManejoHijas}
                                        />
                                        <span className="modalCombinarCampoHint">
                                            {manejoHijas === 'mover'
                                                ? 'Las subcolecciones pasarán a ser hijas de esta colección'
                                                : 'Los samples de las subcolecciones se fusionarán en esta colección'
                                            }
                                        </span>
                                    </div>
                                )}

                                <div className="modalCombinarResumen">
                                    Los <strong>{coleccionOrigen.totalSamples}</strong> samples de
                                    &ldquo;{coleccionOrigen.nombre}&rdquo; se moverán aquí.
                                    Esta acción se puede deshacer durante 7 días.
                                </div>
                            </>
                        )}
                    </>
                )}

                <ModalAcciones>
                    <BotonBase variante="secundario" onClick={onCerrar}>
                        Cancelar
                    </BotonBase>
                    <BotonBase
                        variante="primario"
                        onClick={manejarCombinar}
                        disabled={!origenId || combinando || cargando}
                    >
                        {combinando ? 'Combinando...' : 'Combinar'}
                    </BotonBase>
                </ModalAcciones>
            </div>
        </Modal>
    );
};

export default ModalCombinarColeccion;
