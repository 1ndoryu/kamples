import { FolderOpen, Music2 } from 'lucide-react';
import { BotonBase } from '@app/components/ui/boton/BotonBase';
import type { usePanelSincronizacion } from '@app/hooks/panel/usePanelSincronizacion';
import type { EstadoSampleHistorial } from '@app/stores/varios-4/syncStore';
import { abrirArchivoEnExplorador } from './accionesVentana';
import { claseEstadoSample, etiquetaEstadoSample, iconoEstadoSample } from './estadoVisual';
import { resolverMiniaturaHistorial } from './portadas';
import { repararMojibake } from './textoSync';

type PanelSync = ReturnType<typeof usePanelSincronizacion>;

interface ListaHistorialSyncProps extends Pick<
    PanelSync,
    'historialSamples' | 'carpetaLocal' | 'elegirCarpeta' | 'alternarSincronizacion' | 'sincronizarAhora'
> {
    sincronizacionActiva: boolean;
}

/* Contenido principal: historial per-sample o estado vacio contextual. */
export function ListaHistorialSync({
    historialSamples,
    carpetaLocal,
    sincronizacionActiva,
    elegirCarpeta,
    alternarSincronizacion,
    sincronizarAhora,
}: ListaHistorialSyncProps): JSX.Element {
    return (
        <div className="ventanaSincPanelContenido sincPanelMinimalContenido">
            <div className="sincPanelHistorialMinimal">
                {historialSamples.length === 0 ? (
                    <div className="sincPanelHistorialVacio" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: '8px', height: '100%', textAlign: 'center' }}>
                        {/* [2003A-38] Estado vacío dinámico según contexto */}
                        {!carpetaLocal ? (
                            <>
                                <span>Selecciona una carpeta para sincronizar</span>
                                <BotonBase variante="secundario" tamano="sm" onClick={elegirCarpeta} type="button">
                                    <FolderOpen size={14} /> Elegir carpeta
                                </BotonBase>
                            </>
                        ) : !sincronizacionActiva ? (
                            <>
                                <span>Sincronización pausada</span>
                                <BotonBase variante="secundario" tamano="sm" onClick={alternarSincronizacion} type="button">
                                    Activar sincronización
                                </BotonBase>
                            </>
                        ) : (
                            <>
                                <span>Sin actividad reciente</span>
                                <BotonBase variante="secundario" tamano="sm" onClick={sincronizarAhora} type="button">
                                    Sincronizar ahora
                                </BotonBase>
                            </>
                        )}
                    </div>
                ) : (
                    historialSamples.map((entrada) => (
                        <div
                            key={`${entrada.sampleId}-${entrada.nombreArchivo}`}
                            className={`sincPanelHistorialItemMinimal ${entrada.rutaLocal ? 'sincPanelHistorialClickable' : ''}`}
                            onClick={entrada.rutaLocal ? () => abrirArchivoEnExplorador(entrada.rutaLocal!) : undefined}
                            role={entrada.rutaLocal ? 'button' : undefined}
                            tabIndex={entrada.rutaLocal ? 0 : undefined}
                            onKeyDown={entrada.rutaLocal ? (e) => { if (e.key === 'Enter') abrirArchivoEnExplorador(entrada.rutaLocal!); } : undefined}
                        >
                            <div className="sincPanelHistorialMedia">
                                {resolverMiniaturaHistorial(entrada) ? (
                                    <img
                                        className="sincPanelHistorialThumb"
                                        src={resolverMiniaturaHistorial(entrada) ?? undefined}
                                        alt={repararMojibake(entrada.nombreArchivo)}
                                    />
                                ) : (
                                    <div className="sincPanelHistorialThumb sincPanelHistorialThumbFallback">
                                        <Music2 size={14} />
                                    </div>
                                )}
                            </div>

                            <div className="sincPanelHistorialContenido">
                                <span className="sincPanelHistorialDescMinimal">
                                    {repararMojibake(entrada.nombreArchivo)}
                                </span>
                                <span className={`sincPanelHistorialEstadoLabel ${claseEstadoSample(entrada.estado)}`}>
                                    {etiquetaEstadoSample(entrada.estado as EstadoSampleHistorial)}
                                    {entrada.error ? ` — ${entrada.error}` : ''}
                                </span>
                            </div>

                            <div className="sincPanelHistorialEstadoFinal">
                                {iconoEstadoSample(entrada.estado as EstadoSampleHistorial)}
                            </div>
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
