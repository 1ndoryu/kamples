/*
 * [2003A-16] apiVersiones — Servicio para gestión de versiones de app desde el VPS.
 * Las versiones se almacenan en WP options y se actualizan via el endpoint admin.
 * Sin dependencia de GitHub — todo lo gestiona el propio servidor el propio servidor.
 */

import { apiPeticion } from '../varios-1/apiCliente';
import type { VersionesDisponibles } from '@app/stores/varios-4/versionStore';

export const guardarVersionesAdmin = (versiones: VersionesDisponibles) =>
    apiPeticion<{ ok: boolean }>('/kamples/v1/admin/app/versions', {
        method: 'POST',
        body: versiones,
    });
