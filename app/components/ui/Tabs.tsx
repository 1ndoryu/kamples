'use client';

// app/components/ui/Tabs.tsx
import {useState, type ReactNode} from 'react';

// Define la estructura de cada pestaña que recibirá el componente
export interface Pestaña {
    etiqueta: string;
    contenido: ReactNode;
}

interface Props {
    pestañas: Pestaña[];
    posicionInicial?: number;
}

export default function Tabs({pestañas, posicionInicial = 0}: Props) {
    const [pestañaActiva, setPestañaActiva] = useState(posicionInicial);

    // Prevenimos renderizar si no hay pestañas para evitar errores
    if (!pestañas || pestañas.length === 0) {
        return null;
    }

    return (
        <div className="contenedorTabs">
            <div className="listaPestañas" role="tablist">
                {pestañas.map((pestaña, index) => (
                    <button key={index} role="tab" aria-selected={pestañaActiva === index} aria-controls={`panel-${index}`} id={`pestaña-${index}`} className={`pestañaBoton ${pestañaActiva === index ? 'activa' : ''}`} onClick={() => setPestañaActiva(index)}>
                        {pestaña.etiqueta}
                    </button>
                ))}
            </div>

            <div className="panelContenedor">
                {pestañas.map((pestaña, index) => (
                    <div key={index} role="tabpanel" aria-labelledby={`pestaña-${index}`} id={`panel-${index}`} className="panelPestaña" hidden={pestañaActiva !== index}>
                        {/* Renderizamos el contenido solo si la pestaña está activa */}
                        {pestañaActiva === index && pestaña.contenido}
                    </div>
                ))}
            </div>

            {/* CSS mínimo y encapsulado */}
            <style jsx>{`
                .listaPestañas {
                    display: flex;
                    border-bottom: 1px solid var(--color-borde);
                    margin-bottom: 1.5rem;
                }
                .pestañaBoton {
                    padding: 0.75rem 1.25rem;
                    cursor: pointer;
                    background-color: transparent;
                    border: none;
                    border-bottom: 2px solid transparent;
                    color: var(--color-texto);
                    opacity: 0.7;
                    margin-bottom: -1px; /* Alinea el borde inferior con el del contenedor */
                    transition: all 0.2s ease-in-out;
                }
                .pestañaBoton:hover {
                    background-color: var(--color-tarjeta-fondo-hover);
                    opacity: 1;
                }
                .pestañaBoton.activa {
                    border-bottom-color: var(--color-primario);
                    opacity: 1;
                    font-weight: 600;
                }
                .panelPestaña {
                    /* Estilos para el contenido, si fueran necesarios */
                }
            `}</style>
        </div>
    );
}
