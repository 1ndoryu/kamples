/* sentinel-disable-file limite-lineas — adaptador puente legacy WP → Rust para desktop */
/*
 * [254A-7d] Adaptador desktop: cuando la app desktop apunta a un backend Rust
 * (no al WordPress legacy de kamples.com), reescribe las URLs estilo
 * `${BASE}/wp-json/kamples/v1/X` a `${BASE}/api/{mapPath(X)}` y adapta los
 * bodies y respuestas que difieren entre el contrato camelCase/email del
 * legacy y el snake_case/identifier del backend Rust.
 *
 * Es la version desktop del `frontend/src/bootstrap/wpJsonStub.ts`, con la
 * diferencia clave de que aqui las URLs son absolutas (https://host/...)
 * y NO same-origin: hay que preservar el host original al reescribir el path.
 *
 * Activacion: solo se instala cuando `KAMPLES_BACKEND === 'rust'` (env var
 * inyectada por Vite en build time). Cuando esta en 'wp' (default), el
 * comportamiento es completamente transparente — no se toca el fetch.
 *
 * Endpoints todavia no implementados en Rust (ver 254A-7a/b/c) responden
 * 404 real, lo que el cliente ya maneja como `{ ok: false }`. La UI mostrara
 * estados vacios o errores de red — no rompe el render.
 */

/* PATH_MAP / mapPath identicos a wpJsonStub.ts pero exportados desde aqui
 * para que desktop no dependa del paquete frontend. */
const PATH_MAP: Record<string, string> = {
    '/auth/registro': '/auth/register',
    '/me':            '/users/me',
};

const PATH_MAP_KEEP: Set<string> = new Set([
    '/me/bloqueados',
    /* Endpoints especificos del watcher desktop — mantienen su nombre porque
     * la implementacion Rust (cuando exista) los ofrecera bajo /api/me/sync/*
     * y /api/me/coleccionados/*. Ver 254A-7a/7b/7c. */
    '/me/sync/delta',
    '/me/sync/colecciones',
    '/me/coleccionados',
    '/me/coleccionados/carpetas',
]);

function mapPath(legacyPath: string): string {
    if (PATH_MAP[legacyPath] !== undefined) return PATH_MAP[legacyPath];
    if (PATH_MAP_KEEP.has(legacyPath)) return legacyPath;
    /* /me/coleccionados/{id}/carpeta — tambien se mantiene */
    if (/^\/me\/coleccionados\/\d+\/carpeta$/.test(legacyPath)) return legacyPath;
    if (legacyPath.startsWith('/me/')) return legacyPath.replace('/me/', '/users/me/');
    if (legacyPath.startsWith('/perfil/')) return legacyPath.replace('/perfil/', '/users/');
    return legacyPath;
}

function snakeToCamel(key: string): string {
    return key.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());
}

function convertKeys(obj: unknown): unknown {
    if (Array.isArray(obj)) return obj.map(convertKeys);
    if (obj !== null && typeof obj === 'object') {
        const out: Record<string, unknown> = {};
        for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
            out[snakeToCamel(k)] = convertKeys(v);
        }
        return out;
    }
    return obj;
}

/* Adaptaciones de body/respuesta — solo los endpoints donde el contrato
 * difiere materialmente. El resto pasa tal cual y depende del cliente
 * desktop tolerar snake_case (la mayoria de codepaths ya lo hacen). */
function adaptarBody(rustPath: string, bodyText: string): string {
    if (!bodyText) return bodyText;
    try {
        const payload = JSON.parse(bodyText) as Record<string, unknown>;
        if (rustPath === '/auth/login') {
            if (payload.email !== undefined && payload.identifier === undefined) {
                payload.identifier = payload.email;
                delete payload.email;
            }
            return JSON.stringify(payload);
        }
        if (rustPath === '/auth/register') {
            if (payload.nombreVisible !== undefined && payload.nombre_visible === undefined) {
                payload.nombre_visible = payload.nombreVisible;
                delete payload.nombreVisible;
            }
            return JSON.stringify(payload);
        }
        /* PATCH /users/me: convertir camelCase del frontend a snake_case que Rust espera */
        if (rustPath === '/users/me') {
            if (payload.nombreVisible !== undefined && payload.nombre_visible === undefined) {
                payload.nombre_visible = payload.nombreVisible;
                delete payload.nombreVisible;
            }
            if (payload.sitioWeb !== undefined && payload.sitio_web === undefined) {
                payload.sitio_web = payload.sitioWeb;
                delete payload.sitioWeb;
            }
            if (payload.avatarUrl !== undefined && payload.avatar_url === undefined) {
                payload.avatar_url = payload.avatarUrl;
                delete payload.avatarUrl;
            }
            if (payload.portadaUrl !== undefined && payload.portada_url === undefined) {
                payload.portada_url = payload.portadaUrl;
                delete payload.portadaUrl;
            }
            if (payload.generosPreferidos !== undefined && payload.generos_favoritos === undefined) {
                payload.generos_favoritos = payload.generosPreferidos;
                delete payload.generosPreferidos;
            }
            /* Campos no soportados por UpdateProfileRequest: evitar que viajen como ruido. */
            delete payload.username;
            delete payload.paypalEmail;
            return JSON.stringify(payload);
        }
    } catch { /* JSON invalido — pasar tal cual */ }
    return bodyText;
}

function adaptarUsuario(rustUser: Record<string, unknown>): Record<string, unknown> {
    const u = convertKeys(rustUser) as Record<string, unknown>;
    if ('createdAt' in u) {
        u.creadoAt = u.createdAt;
        u.actualizadoAt = u.updatedAt ?? u.createdAt;
        delete u.createdAt;
        delete u.updatedAt;
    }
    if ('generosFavoritos' in u) {
        u.generosPreferidos = Array.isArray(u.generosFavoritos) ? u.generosFavoritos : [];
        delete u.generosFavoritos;
    }
    u.wpUserId      = u.wpUserId      ?? u.id ?? 0;
    u.bio           = u.bio           ?? '';
    u.avatarUrl     = u.avatarUrl     ?? null;
    u.portadaUrl    = u.portadaUrl    ?? null;
    u.generosPreferidos = u.generosPreferidos ?? [];
    return u;
}

function adaptarRespuesta(rustPath: string, json: unknown): unknown {
    if (json === null || json === undefined || typeof json !== 'object') return json;
    if (Array.isArray(json)) return json;
    const obj = json as Record<string, unknown>;
    if (
        rustPath === '/auth/login' ||
        rustPath === '/auth/register' ||
        rustPath.startsWith('/auth/google')
    ) {
        if (obj.user && !obj.usuario) {
            return {
                ...obj,
                usuario: adaptarUsuario(obj.user as Record<string, unknown>),
                user: undefined,
            };
        }
    }
    /* /users/me: PrivateProfileResponse (snake_case) → UsuarioAutenticado (camelCase) */
    if (rustPath === '/users/me') {
        return adaptarUsuario(obj);
    }
    return obj;
}

/* Reescribe una URL absoluta o relativa que apunta a /wp-json/kamples/v1/X
 * a /api/{mapPath(X)}, preservando el host. */
function reescribirUrl(originalUrl: string): { rustUrl: string; rustPath: string } | null {
    /* Buscar /wp-json/kamples/v1/ en la URL */
    const match = /^(.*?)(\/wp-json)\/kamples\/v1(\/.*)?$/.exec(originalUrl);
    if (!match) return null;
    const baseSinWpJson = match[1]; /* https://host  o  vacio si era relativa */
    const restoConQuery = match[3] ?? '';
    const [legacyPath, query = ''] = restoConQuery.split('?');
    const rustPath = mapPath(legacyPath || '');
    const queryStr = query ? `?${query}` : '';
    const rustUrl = `${baseSinWpJson}/api${rustPath}${queryStr}`;
    return { rustUrl, rustPath };
}

/* Instala el interceptor de fetch que aplica la traduccion. Idempotente. */
let instalado = false;
export function instalarRustAdapter(): void {
    if (instalado) return;
    instalado = true;

    const fetchOriginal = window.fetch.bind(window);

    window.fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
        const url =
            typeof input === 'string'
                ? input
                : input instanceof URL
                    ? input.href
                    : input instanceof Request
                        ? input.url
                        : String(input);

        /* Solo interceptamos rutas estilo /wp-json/kamples/v1/* */
        if (!url.includes('/wp-json/kamples/v1')) {
            return fetchOriginal(input, init);
        }

        const reescrita = reescribirUrl(url);
        if (!reescrita) {
            return fetchOriginal(input, init);
        }

        /* Adaptar body si aplica */
        let bodyText = '';
        if (init?.body && typeof init.body === 'string') {
            bodyText = init.body;
        }
        const adaptedBody = adaptarBody(reescrita.rustPath, bodyText);

        /* Limpiar headers WP-Nonce; preservar Authorization si vino del interceptor anterior */
        const headers = new Headers((init?.headers as HeadersInit | undefined) ?? {});
        headers.delete('X-WP-Nonce');

        const newInit: RequestInit = { ...init, headers };
        if (adaptedBody !== bodyText && adaptedBody !== '') {
            newInit.body = adaptedBody;
        }

        let resp: Response;
        try {
            resp = await fetchOriginal(reescrita.rustUrl, newInit);
        } catch (err) {
            return Promise.reject(err);
        }

        /* Adaptar respuestas de auth (user → usuario) y perfil propio (snake_case → camelCase) */
        const needsAdaptation =
            reescrita.rustPath === '/auth/login' ||
            reescrita.rustPath === '/auth/register' ||
            reescrita.rustPath.startsWith('/auth/google') ||
            reescrita.rustPath === '/users/me';

        if (!needsAdaptation) return resp;

        const text = await resp.text();
        let json: unknown;
        try { json = JSON.parse(text); } catch { return new Response(text, { status: resp.status, headers: resp.headers }); }
        const adapted = adaptarRespuesta(reescrita.rustPath, json);
        return new Response(JSON.stringify(adapted), { status: resp.status, headers: resp.headers });
    };
}
