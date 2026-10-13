/*
 * config.tsx — Entry point de la ventana independiente de configuracion sync.
 * Ventana Tauri creada dinámicamente (frameless, centrada, movible).
 * Renderiza VentanaConfigSync directamente con React.
 *
 * NO inicializa syncService ni auth: solo necesita acceso al Tauri Store
 * para leer/escribir la configuración avanzada de sync.
 */

/* Variables + estilos reutilizados */
import '@app/styles/variables.css';
import '@app/styles/componentes/boton/botonBase.css';
import '@app/styles/componentes/configuracion/configuracionSync.css';
import '@app/styles/componentes/varios-1/campoTexto.css';

/* CSS shell de la ventana config */
import './config.css';

/* Componente standalone de configuracion */
import { VentanaConfigSync } from './components/VentanaConfigSync';

import { createRoot } from 'react-dom/client';
import { elementoPorId } from '@/platform/documento';

function inicializar(): void {
    const contenedor = elementoPorId('config-root');
    if (!contenedor) return;

    const root = createRoot(contenedor);
    root.render(<VentanaConfigSync />);
}

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => { inicializar(); });
} else {
    inicializar();
}
