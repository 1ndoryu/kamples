function formatearTiempoRelativo(timestamp: number): string {
    const seg = Math.floor((Date.now() - timestamp) / 1000);
    if (seg < 60) return 'ahora';
    const min = Math.floor(seg / 60);
    if (min < 60) return `${min}m`;
    const horas = Math.floor(min / 60);
    if (horas < 24) return `${horas}h`;
    return `${Math.floor(horas / 24)}d`;
}

/* [2003A-38] Estados informativos para el usuario */
export function estadoLabel(estado: string): string {
    switch (estado) {
        case 'sincronizando': return 'Sincronizando';
        case 'completado': return 'Sincronizado';
        case 'error': return 'Error';
        case 'pausado': return 'Pausado';
        case 'escaneando': return 'Escaneando';
        default: return 'Listo';
    }
}

export function capitalizarPrimera(texto: string): string {
    if (!texto) return texto;
    return texto.charAt(0).toUpperCase() + texto.slice(1);
}

export function repararMojibake(texto: string): string {
    if (!texto) return '';
    if (!/[ÃÂâ]/.test(texto)) return texto;

    try {
        const bytes = new Uint8Array([...texto].map(c => c.charCodeAt(0) & 0xff));
        const decodificado = new TextDecoder('utf-8').decode(bytes);
        return decodificado.includes('�') ? texto : decodificado;
    } catch {
        return texto;
    }
}
