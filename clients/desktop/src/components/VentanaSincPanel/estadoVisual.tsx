import {
    AlertCircle,
    CheckCircle2,
    CircleDotDashed,
    Loader2,
    Music2,
    PauseCircle,
} from 'lucide-react';
import type { EstadoSampleHistorial } from '@app/stores/varios-4/syncStore';
import { capitalizarPrimera } from './textoSync';

export function iconoEstado(estado: string): JSX.Element {
    switch (estado) {
        case 'sincronizando': return <Loader2 size={14} className="sincPanelSpinner" />;
        case 'completado': return <CheckCircle2 size={14} />;
        case 'error': return <AlertCircle size={14} />;
        case 'pausado': return <PauseCircle size={14} />;
        default: return <CircleDotDashed size={14} />;
    }
}

/* Helpers para el historial per-sample */

export function etiquetaEstadoSample(estado: EstadoSampleHistorial | string): string {
    switch (estado) {
        case 'detectado': return 'Detectado';
        case 'subiendo': return 'Subiendo';
        case 'sincronizado': return 'Sincronizado';
        case 'error': return 'Error';
        case 'moviendo': return 'Moviendo';
        case 'descargando': return 'Descargando';
        case 'descargado': return 'Descargado';
        default: return capitalizarPrimera(estado);
    }
}

export function iconoEstadoSample(estado: EstadoSampleHistorial | string): JSX.Element {
    switch (estado) {
        case 'detectado': return <CircleDotDashed size={13} />;
        case 'subiendo':
        case 'descargando':
        case 'moviendo':
            return <Loader2 size={13} className="sincPanelSpinner" />;
        case 'sincronizado':
        case 'descargado':
            return <CheckCircle2 size={13} />;
        case 'error': return <AlertCircle size={13} />;
        default: return <Music2 size={13} />;
    }
}

export function claseEstadoSample(estado: EstadoSampleHistorial | string): string {
    switch (estado) {
        case 'error': return 'sincPanelEstadoError';
        case 'subiendo':
        case 'descargando':
        case 'moviendo':
            return 'sincPanelEstadoProgreso';
        case 'sincronizado':
        case 'descargado':
            return 'sincPanelEstadoOk';
        default: return '';
    }
}
