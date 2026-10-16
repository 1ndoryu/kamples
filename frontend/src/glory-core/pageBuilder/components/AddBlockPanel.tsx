/**
 * AddBlockPanel - Panel para agregar nuevos bloques
 *
 * Muestra los tipos de bloque disponibles y permite
 * agregar nuevos bloques a la pagina.
 *
 * Usa BlockRegistry para obtener los tipos disponibles.
 */

import {BlockRegistry} from '../BlockRegistry';
import '../styles/constructorPaginas.css';

interface AddBlockPanelProps {
    /** Callback al agregar un bloque */
    onAddBlock: (blockType: string) => void;
    /** Titulo del panel (default: "Agregar Bloque") */
    title?: string;
    /** Solo mostrar ciertos tipos de bloque */
    allowedTypes?: string[];
}

export function AddBlockPanel({onAddBlock, title = 'Agregar Bloque', allowedTypes}: AddBlockPanelProps): JSX.Element {
    const availableTypes = BlockRegistry.getAvailableTypes();

    /* Filtrar tipos si se especifican allowedTypes */
    const typesToShow = allowedTypes ? availableTypes.filter(t => allowedTypes.includes(t.type)) : availableTypes;

    if (typesToShow.length === 0) {
        return (
            <div id="add-block-panel-empty" className="pageBuilderVacio">
                No hay tipos de bloque disponibles
            </div>
        );
    }

    return (
        <div id="add-block-panel" className="pageBuilderPanel">
            {title && (
                <p className="pageBuilderPanelTitulo">
                    {title}
                </p>
            )}

            <div className="pageBuilderPanelTipos">
                {typesToShow.map(blockType => (
                    <button
                        key={blockType.type}
                        type="button"
                        onClick={() => onAddBlock(blockType.type)}
                        className="botonBase pageBuilderTipoBoton">
                        <span className="pageBuilderTipoMas">+</span>
                        {blockType.label}
                    </button>
                ))}
            </div>
        </div>
    );
}

export default AddBlockPanel;
