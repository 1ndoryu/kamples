/**
 * EditModeToggle - Boton flotante para activar modo edicion
 *
 * Muestra un boton flotante que permite a los admins
 * entrar al modo de edicion del Page Builder.
 *
 * Solo se renderiza si canEdit es true.
 */

interface EditModeToggleProps {
    /** Callback al activar modo edicion */
    onActivate: () => void;
    /** Si el usuario puede editar */
    canEdit: boolean;
    /** Texto del boton (default: "Editar Pagina") */
    text?: string;
    /** Posicion del boton */
    position?: 'bottom-right' | 'bottom-left' | 'top-right' | 'top-left';
}

export function EditModeToggle({onActivate, canEdit, text = 'Editar Pagina', position = 'bottom-right'}: EditModeToggleProps): JSX.Element | null {
    if (!canEdit) return null;

    const posiciones: Record<string, string> = {
        'bottom-right': 'pageBuilderToggleEdicion--abajoDerecha',
        'bottom-left': 'pageBuilderToggleEdicion--abajoIzquierda',
        'top-right': 'pageBuilderToggleEdicion--arribaDerecha',
        'top-left': 'pageBuilderToggleEdicion--arribaIzquierda'
    };

    return (
        <button
            id="page-builder-edit-toggle"
            type="button"
            onClick={onActivate}
            className={`botonBase pageBuilderToggleEdicion ${posiciones[position]}`}>
            {text}
        </button>
    );
}

export default EditModeToggle;
