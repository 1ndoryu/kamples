/*
 * Componente: TabHistorialLotesAdmin — [223A-3]
 * Historial de lotes de automatización (extracción audio + scraper WhoSampled).
 * Muestra estado de automatización, botones de reactivación y tabla de lotes.
 */

import { useState } from 'react';
import { RefreshCw, Loader2, ChevronLeft, ChevronRight } from 'lucide-react';
import { BotonBase } from '../../../ui/boton/BotonBase';
import { Badge } from '../../../ui/badge/Badge';
import { SelectorMenu } from '../../../ui/selector/SelectorMenu';
import { EstadoVacio } from '../../../ui/varios-1/EstadoVacio';
import { useHistorialLotes } from '../../../../hooks/historial/useHistorialLotes';
import type { TipoProceso } from '../../../../services/api/varios-1/apiAutomatizacion';
import type { OpcionSelector } from '../../../ui/selector/SelectorMenu';
import { AutomationBatchDetails } from '../../automation/AutomationBatchDetails';
import { AutomationProcessCard } from '../../automation/AutomationProcessCard';
import '../../../../styles/componentes/admin/adminTablas.css';

const POR_PAGINA = 20;

const OPCIONES_TIPO: OpcionSelector[] = [
    { valor: '', etiqueta: 'Todos' },
    { valor: 'extraccion', etiqueta: 'Extracción Audio' },
    { valor: 'scraping', etiqueta: 'Scraper WhoSampled' },
];

type VarianteBadge = 'neutro' | 'acento' | 'exito' | 'error' | 'advertencia' | 'info';

const colorEstadoLote = (estado: string): VarianteBadge => {
    const mapa: Record<string, VarianteBadge> = {
        ejecutando: 'info',
        completado: 'exito',
        error: 'error',
        detenido: 'advertencia',
    };
    return mapa[estado] ?? 'neutro';
};

const formatearFecha = (fecha: string | null): string => {
    if (!fecha) return '—';
    const d = new Date(fecha);
    return d.toLocaleDateString('es', { day: '2-digit', month: 'short' }) +
        ' ' + d.toLocaleTimeString('es', { hour: '2-digit', minute: '2-digit' });
};

export const TabHistorialLotesAdmin = (): JSX.Element => {
    const hist = useHistorialLotes();
    const [reactivando, setReactivando] = useState<TipoProceso | null>(null);

    const totalPaginas = Math.ceil(hist.total / POR_PAGINA);

    const manejarReactivar = async (tipo: TipoProceso) => {
        setReactivando(tipo);
        await hist.reactivar(tipo);
        setReactivando(null);
    };

    return (
        <div className="adminTablaContenedor">
            {/* Estado de automatización */}
            {hist.estado && (
                <div className="adminTablaBarraSuperior">
                    <AutomationProcessCard
                        titulo="Extracción Audio"
                        tipo="extraccion"
                        activo={hist.estado.extraccion.activo}
                        limiteLote={hist.estado.extraccion.limite_por_lote}
                        intervaloSegundos={hist.estado.extraccion.intervalo_segundos}
                        ultimoLote={hist.estado.extraccion.ultimo_lote}
                        proceso={hist.procesos.extraccion}
                        reactivando={reactivando === 'extraccion'}
                        guardando={hist.guardandoConfig === 'extraccion'}
                        forzando={hist.forzandoProceso === 'extraccion'}
                        onReactivar={() => manejarReactivar('extraccion')}
                        onForzarEjecucion={(limiteLote) => hist.forzarProceso('extraccion', limiteLote)}
                        onGuardarConfig={(config) => hist.guardarConfig('extraccion', config)}
                    />
                    <AutomationProcessCard
                        titulo="Scraper WhoSampled"
                        tipo="scraping"
                        activo={hist.estado.scraping.activo}
                        limiteLote={hist.estado.scraping.limite_por_lote}
                        intervaloSegundos={hist.estado.scraping.intervalo_segundos}
                        ultimoLote={hist.estado.scraping.ultimo_lote}
                        proceso={hist.procesos.scraping}
                        fallosConsecutivos={hist.estado.scraping.fallos_consecutivos}
                        reactivando={reactivando === 'scraping'}
                        guardando={hist.guardandoConfig === 'scraping'}
                        forzando={hist.forzandoProceso === 'scraping'}
                        onReactivar={() => manejarReactivar('scraping')}
                        onForzarEjecucion={(limiteLote) => hist.forzarProceso('scraping', limiteLote)}
                        onGuardarConfig={(config) => hist.guardarConfig('scraping', config)}
                    />
                </div>
            )}

            {/* Controles */}
            <div className="adminTablaBarraSuperior">
                <SelectorMenu
                    opciones={OPCIONES_TIPO}
                    valor={hist.filtroTipo}
                    onChange={(v) => { hist.setFiltroTipo(v as TipoProceso | ''); hist.setPagina(1); }}
                    placeholder="Filtrar por tipo"
                />
                <BotonBase variante="ghost" tamano="sm" onClick={hist.refrescar} title="Refrescar">
                    <RefreshCw size={14} />
                </BotonBase>
            </div>

            {/* Tabla de lotes */}
            {hist.cargando ? (
                <div className="adminTablaCargando"><Loader2 size={20} className="animacionGiro" /> Cargando...</div>
            ) : hist.lotes.length === 0 ? (
                <EstadoVacio mensaje="No hay lotes registrados" />
            ) : (
                <>
                    <table className="adminTabla">
                        <thead>
                            <tr>
                                <th>ID</th>
                                <th>Tipo</th>
                                <th>Estado</th>
                                <th>Inicio</th>
                                <th>Éxitos</th>
                                <th>Fallos</th>
                                <th>Detalles</th>
                            </tr>
                        </thead>
                        <tbody>
                            {hist.lotes.map(lote => (
                                <tr key={lote.id}>
                                    <td>{lote.id}</td>
                                    <td>
                                        <Badge variante={lote.tipo === 'extraccion' ? 'acento' : 'info'}>
                                            {lote.tipo === 'extraccion' ? 'Extracción' : 'Scraping'}
                                        </Badge>
                                    </td>
                                    <td>
                                        <Badge variante={colorEstadoLote(lote.estado)}>{lote.estado}</Badge>
                                    </td>
                                    <td className="celdaCompacta">{formatearFecha(lote.iniciado_at)}</td>
                                    <td className="celdaNumero">{lote.exitosos}</td>
                                    <td className="celdaNumero">{lote.fallidos}</td>
                                    <td className="celdaCompacta">
                                        <AutomationBatchDetails lote={lote} />
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>

                    {/* Paginación */}
                    {totalPaginas > 1 && (
                        <div className="adminTablaPaginacion">
                            <BotonBase
                                variante="ghost" tamano="sm"
                                onClick={() => hist.setPagina(p => Math.max(1, p - 1))}
                                disabled={hist.pagina <= 1}
                            >
                                <ChevronLeft size={14} />
                            </BotonBase>
                            <span>{hist.pagina} / {totalPaginas}</span>
                            <BotonBase
                                variante="ghost" tamano="sm"
                                onClick={() => hist.setPagina(p => Math.min(totalPaginas, p + 1))}
                                disabled={hist.pagina >= totalPaginas}
                            >
                                <ChevronRight size={14} />
                            </BotonBase>
                        </div>
                    )}
                </>
            )}
        </div>
    );
};

