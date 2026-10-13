/* [284A-2] Tarjeta editable de automatizacion admin.
 * Por que: el admin necesita ahorrar ruido visual, ver la proxima corrida
 * estimada y poder disparar un lote manual sin salir del panel. */

import { AlertTriangle, CheckCircle, Loader2, PauseCircle, Play, RotateCcw, Save } from 'lucide-react';
import { Badge } from '../../ui/badge/Badge';
import { BotonBase } from '../../ui/boton/BotonBase';
import { Input } from '../../ui/input/Input';
import type { AutomatizacionConfigProceso, LoteResumen, TipoProceso } from '../../../services/api/varios-1/apiAutomatizacion';
import type { EstadoProceso } from '../../../services/api/varios-2/apiProcesos';
import { useAutomationProcessCard } from '../../../hooks/varios-1/useAutomationProcessCard';

interface AutomationProcessCardProps {
    titulo: string;
    tipo: TipoProceso;
    activo: boolean;
    limiteLote: number;
    intervaloSegundos: number;
    ultimoLote: LoteResumen | null;
    proceso: EstadoProceso | undefined;
    fallosConsecutivos?: number;
    reactivando: boolean;
    guardando: boolean;
    forzando: boolean;
    onReactivar: () => void;
    onForzarEjecucion: (limiteLote: number) => Promise<unknown>;
    onGuardarConfig: (config: Required<AutomatizacionConfigProceso>) => Promise<unknown>;
}

export const AutomationProcessCard = ({
    titulo,
    tipo,
    activo,
    limiteLote,
    intervaloSegundos,
    ultimoLote,
    proceso,
    fallosConsecutivos,
    reactivando,
    guardando,
    forzando,
    onReactivar,
    onForzarEjecucion,
    onGuardarConfig,
}: AutomationProcessCardProps): JSX.Element => {
    const card = useAutomationProcessCard({
        tipo,
        activo,
        limiteLote,
        intervaloSegundos,
        ultimoLote,
        proceso,
        onGuardarConfig,
    });

    return (
        <form className="tarjetaEstadoProceso" onSubmit={card.manejarSubmit}>
            <div className="tarjetaEstadoEncabezado">
                {activo
                    ? <CheckCircle size={12} className="iconoEstadoProceso iconoEstadoProcesoExito" />
                    : <PauseCircle size={12} className="iconoEstadoProceso iconoEstadoProcesoError" />}
                <strong>{titulo}</strong>
                <Badge variante={activo ? 'exito' : 'error'}>{activo ? 'Habilitado' : 'Deshabilitado'}</Badge>
            </div>

            <div className="tarjetaEstadoInfo">
                {limiteLote} items/lote · {card.intervaloLegible}
                {fallosConsecutivos !== undefined && fallosConsecutivos > 0 && (
                    <span> · <AlertTriangle size={12} className="iconoAlertaLote" /> {fallosConsecutivos} fallos consecutivos</span>
                )}
            </div>

            {ultimoLote ? (
                <div className="tarjetaEstadoUltimo">
                    Último: {ultimoLote.exitosos ?? 0} ok / {ultimoLote.fallidos ?? 0} err - {card.ultimoLoteLegible}
                </div>
            ) : (
                <div className="tarjetaEstadoUltimo">Último: sin registros</div>
            )}

            <div className="tarjetaEstadoProxima">
                Próxima: {card.proximaEjecucionLegible}
            </div>

            <div className="tarjetaEstadoRuntime">
                Proceso: {card.estadoProcesoLegible}
                {card.detalleProcesoLegible ? ` · ${card.detalleProcesoLegible}` : ''}
            </div>

            <div className="tarjetaEstadoControles">
                <BotonBase
                    type="button"
                    variante="ghost"
                    tamano="ninguno"
                    className={`interruptorProceso ${card.enabled ? 'interruptorProcesoActivo' : ''}`}
                    role="switch"
                    aria-checked={card.enabled}
                    onClick={() => card.setEnabled(v => !v)}
                    disabled={guardando}
                >
                    <span className="interruptorProcesoPunto" />
                    <span>{card.enabled ? 'Habilitado' : 'Deshabilitado'}</span>
                </BotonBase>
                <label className="campoConfigProceso">
                    <span>Items/lote</span>
                    <Input
                        type="number"
                        min={1}
                        max={card.limiteMaximo}
                        value={card.lote}
                        onChange={(event) => card.setLote(event.target.value)}
                        required
                    />
                </label>
                <label className="campoConfigProceso">
                    <span>Intervalo (s)</span>
                    <Input
                        type="number"
                        min={card.intervaloMinimo}
                        max={86400}
                        value={card.intervalo}
                        onChange={(event) => card.setIntervalo(event.target.value)}
                        required
                    />
                </label>
                <div className="tarjetaEstadoAcciones">
                    <BotonBase
                        variante="ghost"
                        tamano="sm"
                        soloIcono
                        type="button"
                        title="Forzar ejecución ahora"
                        aria-label="Forzar ejecución ahora"
                        disabled={forzando || guardando || card.configInvalida}
                        onClick={() => onForzarEjecucion(card.loteActual)}
                        className="tarjetaEstadoAccionIcono"
                    >
                        {forzando ? <Loader2 size={14} className="animacionGiro" /> : <Play size={14} />}
                    </BotonBase>
                    <BotonBase
                        variante="secundario"
                        tamano="sm"
                        soloIcono
                        type="submit"
                        title="Guardar configuración"
                        aria-label="Guardar configuración"
                        disabled={guardando || card.configInvalida || !card.hayCambios}
                        className="tarjetaEstadoAccionIcono"
                    >
                        {guardando ? <Loader2 size={14} className="animacionGiro" /> : <Save size={14} />}
                    </BotonBase>
                    {!activo && (
                        <BotonBase
                            variante="primario"
                            tamano="sm"
                            soloIcono
                            type="button"
                            title="Reactivar proceso"
                            aria-label="Reactivar proceso"
                            onClick={onReactivar}
                            disabled={reactivando || guardando || forzando}
                            className="tarjetaEstadoAccionIcono"
                        >
                            {reactivando ? <Loader2 size={14} className="animacionGiro" /> : <RotateCcw size={14} />}
                        </BotonBase>
                    )}
                </div>
            </div>
        </form>
    );
};