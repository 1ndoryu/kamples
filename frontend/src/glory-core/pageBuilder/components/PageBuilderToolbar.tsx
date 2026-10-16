/**
 * PageBuilderToolbar - Toolbar de edicion del Page Builder
 *
 * Componente agnostico que muestra la barra de herramientas
 * cuando el Page Builder esta en modo edicion.
 *
 * Se puede personalizar via props:
 * - onExit: Callback al salir del modo edicion
 * - onSave: Callback al guardar cambios
 * - isSaving: Estado de guardado
 * - title: Titulo opcional
 */

import '../styles/constructorPaginas.css';

interface PageBuilderToolbarProps {
    /** Callback al salir del modo edicion */
    onExit: () => void;
    /** Callback al guardar cambios */
    onSave: () => void;
    /** Si esta guardando actualmente */
    isSaving?: boolean;
    /** Titulo a mostrar (default: "Editando Pagina") */
    title?: string;
    /** Texto del boton salir (default: "Salir") */
    exitText?: string;
    /** Texto del boton guardar (default: "Guardar Cambios") */
    saveText?: string;
    /** Texto mientras guarda (default: "Guardando...") */
    savingText?: string;
}

export function PageBuilderToolbar({onExit, onSave, isSaving = false, title = 'Editando Pagina', exitText = 'Salir', saveText = 'Guardar Cambios', savingText = 'Guardando...'}: PageBuilderToolbarProps): JSX.Element {
    return (
        <div
            id="page-builder-toolbar"
            className="pb-toolbar pageBuilderToolbar">
            <div className="pageBuilderToolbarFila">
                <div className="pageBuilderToolbarIzquierda">
                    <button
                        type="button"
                        onClick={onExit}
                        className="botonBase pageBuilderToolbarSalir">
                        ← {exitText}
                    </button>
                    <span className="pageBuilderToolbarTitulo">
                        {title}
                    </span>
                </div>

                <button
                    type="button"
                    onClick={onSave}
                    disabled={isSaving}
                    className="botonBase pageBuilderToolbarGuardar">
                    {isSaving ? savingText : saveText}
                </button>
            </div>
        </div>
    );
}

export default PageBuilderToolbar;
