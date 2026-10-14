/*
 * Componente: FiltroColumna
 * Dropdown con checkboxes para filtrar valores en columnas de tabla.
 * Reutilizable en cualquier tabla admin que necesite filtros por columna.
 */

import { useState, useRef } from 'react';
import { Filter } from 'lucide-react';
import { Checkbox } from './Checkbox';
import { useCerrarAlClickFuera } from '@app/hooks/menu/useCerrarAlClickFuera';
import '../../../styles/componentes/filtro/filtroColumna.css';

export interface OpcionFiltro {
    valor: string;
    etiqueta: string;
}

interface FiltroColumnaProps {
    opciones: OpcionFiltro[];
    activos: Set<string>;
    onChange: (activos: Set<string>) => void;
}

export const FiltroColumna = ({ opciones, activos, onChange }: FiltroColumnaProps): JSX.Element => {
    const [abierto, setAbierto] = useState(false);
    const refContenedor = useRef<HTMLDivElement>(null);
    const tieneActivos = activos.size > 0 && activos.size < opciones.length;

    useCerrarAlClickFuera(refContenedor, abierto, () => setAbierto(false));

    const toggleOpcion = (valor: string) => {
        const siguiente = new Set(activos);
        if (siguiente.has(valor)) {
            siguiente.delete(valor);
        } else {
            siguiente.add(valor);
        }
        onChange(siguiente);
    };

    return (
        <div className="filtroColumnaContenedor" ref={refContenedor}>
            <button
                type="button"
                className={`filtroColumnaBoton ${tieneActivos ? 'filtroColumnaActivo' : ''}`}
                onClick={(e) => { e.stopPropagation(); setAbierto(prev => !prev); }}
                title="Filtrar columna"
            >
                <Filter size={10} />
            </button>
            {abierto && (
                <div className="filtroColumnaDropdown">
                    <button
                        type="button"
                        className="filtroColumnaLimpiar"
                        onClick={() => onChange(new Set())}
                    >
                        Mostrar todos
                    </button>
                    {opciones.map(op => (
                        <Checkbox
                            key={op.valor}
                            className="filtroColumnaOpcion"
                            checked={activos.size === 0 || activos.has(op.valor)}
                            onChange={() => toggleOpcion(op.valor)}
                            label={op.etiqueta}
                        />
                    ))}
                </div>
            )}
        </div>
    );
};