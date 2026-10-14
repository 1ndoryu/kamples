/*
 * Componente: BuscadorCanciones
 * Buscador con debounce que consulta /canciones/buscar.
 * Muestra resultados como lista seleccionable; expone opcion "nueva cancion".
 * Estado y debounce en useBuscadorCanciones.
 */

import { Search, PlusCircle, X } from 'lucide-react';
import type { Cancion } from '../../../types/varios-1/cancion';
import { InputBusqueda } from '../../ui/input/InputBusqueda';
import { BotonBase } from '../../ui/boton/BotonBase';
import { useT } from '@app/utils/i18n/useT';
import { useBuscadorCanciones } from '@app/hooks/samples/useBuscadorCanciones';
import '../../../styles/componentes/varios-1/buscadorCanciones.css';

interface BuscadorCancionesProps {
    placeholder?: string;
    onSeleccionar: (cancion: Cancion | null) => void;
    onAgregarNueva?: () => void;
    cancionActual?: Cancion | null;
}

export function BuscadorCanciones({
    placeholder = 'Buscar canción por título o artista...',
    onSeleccionar,
    onAgregarNueva,
    cancionActual = null,
}: BuscadorCancionesProps): JSX.Element {
    const { t } = useT();
    const {
        query,
        setQuery,
        resultados,
        cargando,
        abierto,
        setAbierto,
        contenedorRef,
        elegirCancion,
    } = useBuscadorCanciones(onSeleccionar);

    return (
        <div className="buscadorCanciones" ref={contenedorRef}>
            {cancionActual && (
                <div className="buscadorCancionSeleccionada">
                    <div className="buscadorCancionInfo">
                        <span className="buscadorCancionTitulo">{cancionActual.titulo}</span>
                        {cancionActual.artistaNombre && (
                            <span className="buscadorCancionArtista">{cancionActual.artistaNombre}</span>
                        )}
                        {cancionActual.anio && (
                            <span className="buscadorCancionAnio">({cancionActual.anio})</span>
                        )}
                    </div>
                    <BotonBase
                        variante="ghost"
                        tamano="ninguno"
                        className="buscadorCancionQuitar"
                        onClick={() => onSeleccionar(null)}
                        aria-label={t('busqueda.quitarCancion')}
                        type="button"
                    >
                        <X size={14} />
                    </BotonBase>
                </div>
            )}

            <InputBusqueda
                placeholder={placeholder}
                valor={query}
                onChange={setQuery}
                debounceMs={0} /* debounce propio via useEffect */
            />

            {cargando && (
                <div className="buscadorCargando">
                    <Search size={14} />
                    <span>{t('busqueda.buscando')}</span>
                </div>
            )}

            {abierto && !cargando && (
                <ul className="buscadorResultados">
                    {resultados.map((c) => (
                        <li key={c.id} className="buscadorResultadoItem">
                            <BotonBase
                                variante="ghost"
                                tamano="ninguno"
                                type="button"
                                className="buscadorResultadoBoton"
                                onClick={() => elegirCancion(c)}
                            >
                                <span className="buscadorResultadoTitulo">{c.titulo}</span>
                                {c.artistaNombre && (
                                    <span className="buscadorResultadoArtista">{c.artistaNombre}</span>
                                )}
                                {c.anio && (
                                    <span className="buscadorResultadoAnio">({c.anio})</span>
                                )}
                            </BotonBase>
                        </li>
                    ))}

                    {resultados.length === 0 && query.length >= 2 && (
                        <li className="buscadorSinResultados">
                            <span>{t('busqueda.noEncontrado').replace('{q}', query)}</span>
                            {onAgregarNueva && (
                                <BotonBase
                                    variante="ghost"
                                    tamano="ninguno"
                                    type="button"
                                    className="buscadorBotonNueva"
                                    onClick={() => {
                                        setAbierto(false);
                                        onAgregarNueva();
                                    }}
                                >
                                    <PlusCircle size={14} />
                                    Agregar nueva cancion
                                </BotonBase>
                            )}
                        </li>
                    )}
                </ul>
            )}
        </div>
    );
}
