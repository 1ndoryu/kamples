## Stack implementado

| Capa                 | Herramienta                                    |
| -------------------- | ---------------------------------------------- |
| Framework web        | Axum 0.7                                       |
| OpenAPI              | utoipa 4 + utoipa-swagger-ui 7                 |
| Serialización        | serde                                          |
| Base de datos        | SQLx 0.8 (PostgreSQL)                          |
| Migraciones          | SQLx migrate                                   |
| Validación           | validator 0.18                                 |
| Variables de entorno | dotenvy                                        |
| Logging              | tracing + tracing-subscriber                   |
| Errores              | thiserror 2                                    |
| Auth                 | jsonwebtoken + argon2                          |
| CORS                 | tower-http                                     |
| Linter               | clippy (deny all + warn pedantic)              |
| Frontend             | React 18 + TypeScript + Vite                   |
| State                | React Query + Zustand                          |
| Codegen              | Orval 8 (reemplaza openapi-typescript-codegen) |

## Misión principal — Migración Kamples PHP → Rust

**Objetivo:** Portar el proyecto `glorytemplate` (Kamples sobre WordPress + PHP + React Islands) a `glory-rust-template` (Axum + SQLx + React + Vite + Orval), manteniendo paridad funcional total y mejorando arquitectura.

**Origen:** `C:\Users\Owner\OneDrive\Documentos\WP\app\public\wp-content\themes\glorytemplate`
**Destino:** `C:\Users\Owner\OneDrive\Documentos\glory-rust-template`

**Alcance del origen (estimación):**
- ~60 controladores REST (`App/Kamples/Api/Controladores/`)
- ~42 repositorios (`App/Kamples/Database/Repositories/`)
- ~38 servicios de dominio (`App/Kamples/Services/`)
- 74 migraciones SQL acumuladas (`App/Kamples/Database/migrations/v001..v074`)
- Auth propia + Google OAuth (web y mobile) + JWT
- Audio pipeline: FFmpeg, BPM, tonalidad, embeddings pgvector 128d, deduplicación
- IA: moderación (4 capas), generación, embeddings (Groq + OpenAI)
- Pagos Stripe + revenue share + códigos gratis + transacciones idempotentes
- WebSocket Bun standalone (notificaciones tiempo real)
- Push: FCM (Firebase) + Web Push
- Blog completo (artículos, likes, moderación, feed)
- DAW web (Channel Rack + Mixer + Piano Roll)
- Sync changelog + desktop (Tauri) + mobile (Android APK)
- SEO dinámico + sitemaps + JSON-LD
- Algoritmo de descubrimiento (6 señales, planificador, precomputador feed)
- Subcolecciones, contribuciones, mensajes multimedia, reacciones, reportes legales

**Frontend:** Ya existe y NO se recrea desde cero. Se migra del modelo "React Islands en WordPress" a SPA pura (Vite + Orval). Toda la UI/UX, componentes, estilos y stores existentes (`App/React/`) se preservan; lo que cambia es la capa de datos (servicios PHP fetch → cliente Orval generado desde OpenAPI Rust).

**Aclaraciones del usuario (2026-04-17):**
- **Algoritmo de descubrimiento:** debe recrearse en Rust con los mismos detalles del proyecto anterior (6 señales, embeddings 128d, planificador, precomputador feed, selector candidatos). Ver `App/docs (ignorar)/algoritmo.md` y `App/Kamples/Services/{MotorRecomendacion, PlanificadorAlgoritmo, PrecomputadorFeed, SelectorCandidatos, ConstructorSenales, GeneradorEmbeddings}.php` como referencia funcional.
- **Una sola base de datos:** PostgreSQL único (con pgvector). No replicar el split WordPress MySQL + Postgres del legado. Todo va a la BD Postgres del template Rust.
- **Sistemas a portar (incluidos):** scraper (`kamples-scraper/`), mezclador/DAW (`Mezclador/`), mobile WebView (`mobile/` Android), WebSocket (consolidar Bun standalone → Axum WS o mantener Bun según decisión técnica del plan).
- **Frontend ya hecho:** reusar tal cual; solo regenerar capa de servicios desde el cliente Orval.

**Principios de la migración:**
1. Lo agnóstico va a `glory-rs/` (framework reutilizable). Lo específico de Kamples va al proyecto.
2. OpenAPI (`utoipa`) como contrato único. Frontend NO escribe tipos a mano — todo viene de Orval.
3. Migraciones SQLx versionadas; consolidar las 74 migraciones legacy en un schema base limpio + migraciones nuevas a partir de ahí (no replicar la historia).
4. Repositorios PHP → módulos Rust con `sqlx::query_as!` (validación compile-time).
5. Servicios PHP → traits + structs Rust con DI explícita.
6. Controladores PHP → handlers Axum delgados; toda lógica en services.
7. Newtypes para IDs de dominio (`SampleId`, `UserId`, `ColeccionId`...).
8. Cero parches: si el diseño PHP era subóptimo, rediseñar — no portar deuda técnica.

## Pendiente

- **Falso positivo de VarSense `claseHuerfana` con clases que se usan como valor de un mapa de variantes (herramienta, no KAMPLES).** Verificado el 2026-10-15 sobre los 110 hallazgos `claseHuerfana` del gate. El detector solo reconoce el nombre de clase cuando aparece como cadena literal de `className`; no ve las que viven como **valor de una propiedad de objeto** ni como **`return` de un `case`**. 30 casos: `badgeXs…badgePremium` (9, `legacy/components/ui/badge/Badge.tsx:24-34`), `variantePrimario/Ghost/Peligro` + `tamanoMd` (4, `ui/boton/BotonBase.tsx:24-32`), `avatarXs…avatar2xl` + `avatarSm` (6, `ui/varios-1/Avatar.tsx:24-29`), `notificacionExito/Error/Advertencia/Info` (4, `ui/varios-2/Notificacion.tsx:37-42`), `sincPanelEstadoError/Progreso/Ok` (3, `clients/desktop/src/components/VentanaSincPanel/estadoVisual.tsx:54-61`), `modalPequeno/Grande` (2, `ui/modal/Modal.tsx:26-28`), `tarjetaMetaBadgeClickable` y `tarjetaPremium` (2). **Arreglo:** que el analizador acepte el nombre como valor de una propiedad de objeto o en un `return`/`case` de un `.ts`/`.tsx` del proyecto. Desbloqueo: tarea propia en el repositorio de varsense (mismo destino que el `variableNoDefinida` de abajo).
- **`claseHuerfana` sobre `.prose`, `.prose-sm`, `.single-post-container`, `.single-post-featured-image`: se conservan a propósito (herramienta).** Cero productores en el repo (barrido de 1423 ficheros `.tsx/.ts/.jsx/.js/.html/.rs/.php/.twig/.json`: ninguna referencia fuera del propio CSS) y no hay tema WordPress ni PHP en KAMPLES. Pero el comentario de `frontend/src/glory-core/index.css:12-15` declara que existen para "contenido HTML generado por IA y articulos de blog", y ese HTML entra crudo por `dangerouslySetInnerHTML` (`legacy/islands/blog/ArticuloDetalleIsland.tsx:165`) desde el cuerpo del artículo guardado en la BD, que no se puede inspeccionar desde aquí. Borrarlas arriesgaría el render de artículos ya publicados a cambio de 4 hallazgos. **Decidido: se quedan**, y el hallazgo se anota como falso positivo del detector (no ve las clases que llegan dentro de HTML inyectado).
- **Falso positivo de VarSense `variableNoDefinida` con el patrón de tokens que la propia herramienta exige (herramienta, no KAMPLES).** Verificado el 2026-10-15 con 8 casos en `mezcladorMixer.css`/`mezcladorBloques.css`. VarSense exime de `cssInlineReact` los objetos `style={{ '--x': … }}` cuyas claves son **todas** custom properties, y su comentario lo declara "el mismo patron de definicion runtime de tokens que `.style.setProperty('--x', ...)`" (`varsense/src/core/analyzeDocument.ts:190-195`, exención en `:291-299` vía `esSoloDefinicionTokens`, `:272`). Pero el índice de definiciones solo implementa el segundo: `parsearDefinicionesRuntime` (`varsense/src/parsers/cssParser.ts:174-198`) casa únicamente `.setProperty\s*\(\s*(['"`])--([\w-]+)`. Resultado: la conversión que las dos herramientas piden para los estilos inline genera un `variableNoDefinida` nuevo cada vez. **Arreglo:** extraer `REGEX_STYLE_OBJ`, `findMatchingObjectEnd`, `saltarLiteralEstilo`, `extraerClavesObjetoEstilo` y `esSoloDefinicionTokens` de `analyzeDocument.ts` a un módulo compartido (p. ej. `parsers/estiloInline.ts`; hoy `core` ya importa de `parsers`, así que la dirección no puede invertirse) e indexar también esas claves en `parsearDefinicionesRuntime`. **Ojo con dónde se aplica:** el runtime que usa el tablero es `<area>/.quality-tools/varsense/dist/cli/index.js` (`workspace-manager/src/server/gate/ejecucion.ts:93-94`), y su `origin` es `RESTAURANTE/tools/varsense`, no `area-trabajo/varsense` — es decir, el arreglo toca el repositorio de otra herramienta y hay que recompilar el runtime provisionado. **Por eso queda pendiente aquí y no se hace dentro del bloque de KAMPLES** (§3: no mezclar proyectos). Desbloqueo: abrir tarea propia en el repositorio de varsense.
- **15A-2 — Alta de KAMPLES en el taller (VPS 89): HECHO (2026-10-15).** Decidido por la usuaria para poder verificar render (todo el bloque de calidad que queda lo cambia y AGENTS §5 exige evidencia real). Ficha remota en `workspace-manager/scripts/dev/registro.json` (`host: dev89`, puertos 3001/5183, `botonRemoto` 1:1 con los puertos, `healths` con `3001 /api/health`, `precompilar: cargo build --bin glory-backend`, `envRemoto` 9 claves, `buildMode: builder`) y `taller.json` → `proyectos.KAMPLES.basedatos: "kamples"`. Sin `bdDesdeLocal` (no hay BD de KAMPLES en el laptop) y sin `analisisEn` (el analisis sigue en el laptop para no mover los recuentos del gate). `Arranque up` verde: tmux 2/2, tunel y probe 2/2; `/api/samples` → 200 con paginacion y `/api/colecciones` → 401 por auth, contra el esquema real. Dos obstaculos resueltos: (1) la base se llama `kamples` porque `glory_backend` en la 89 es de RESTAURANTE; (2) `CREATE EXTENSION vector` exige superusuario y `dev` no lo era — la usuaria decidio darle superusuario (`ALTER ROLE dev SUPERUSER`, 2026-10-15), y el acceso root de la 89 es `ssh root@89.117.16.170` con la misma clave que `dev89`. `cargo sqlx` no esta en la 89: las 24 migraciones se aplican con `psql -v ON_ERROR_STOP=1 -f` en orden, registrando a mano `_sqlx_migrations` (version/description/checksum sha384) para que un futuro `sqlx migrate run` sea no-op.
- **15A-2b — El juego de migraciones no se podia aplicar desde cero: HECHO (2026-10-15).** `20260417000005_social_engagement.up.sql:66` creaba `colecciones_guardadas` con FK a `colecciones`, que no existe hasta `20260418000030`; al quitarse el `CREATE TABLE colecciones` duplicado de `samples_core` (fix 037A-1) esa FK quedo colgando, y el fallo arrastraba a `mv_trending_samples` (`relation "likes" does not exist`). Se elimina el bloque legacy: la tabla canonica la crea `20260418000031` con `DROP + CREATE` (`coleccion_id BIGINT`), asi que en base nueva el resultado final no cambia. Evidencia: 24/24 OK, 0 fallos, 42 tablas.
- Calidad Sentinel/VarSense KAMPLES (13AA-1): F1 hecho (0 nuevos, 2797 → 2543); F2 mecánica parcial; console-production hecho en escritorio (`3d55e63`) y en `useWebSocket.ts` (`2fcd15a`). Pendiente: console-production de `glory-core` (vendorizado, backlog), tokens y F3 de diseño con verificación visual; plan y backlog en [plan](Agente/planes/plan-calidad-sentinel-kamples-2026-10-13.md). F4 (Rust, 194 errores) pendiente: el cambio de config (`f32fd05`, incluir `*.rs` en Sentinel) no está en `main`. Decidido (13AA-5): no integrarlo aún, porque activar `*.rs` en `main` haría que el gate `--cierre` cuente 194 hallazgos nuevos. Rama `rescate/sentinel-rs-kamples` retirada de local y remoto tras `git bundle verify` (`Agente/backup-ramas-2026-10-13/kamples-rescate-sentinel-rs-kamples.bundle`). Desbloqueo: triaje del lote F4; después reaplicar solo `sentinel.config.json` de `f32fd05` en `main`.
- **15A-1 — Shell SPA huérfano y fichero corrupto en `frontend/src`: HECHO (`0f6a218`).** `frontend/src/{App.tsx,App.css,router.tsx,app/,components/,pages/,hooks/,utils/,styles/}` (52 archivos versionados) era el shell SPA anterior a la migración 204A-1 (2026-04-20). `tsconfig.json` no lo incluía y `index.html` solo carga `/src/main.tsx`, así que nada lo alcanzaba; comprobado además que **ningún archivo de fuera del propio shell lo importaba** (grep de imports relativos a `./components|./pages|./hooks|./utils|./app`: solo aparecía dentro de `App.tsx`/`router.tsx`, que eran parte del shell). Dentro, `components/auth/ModalAuth.tsx` **tenía sintaxis TSX inválida commiteada** (`esbuild`: `63:35 Expected ">" but found "("`), corrupto ya desde su creación (`957ad25`). Borrado el 2026-10-15 tras desactivar la usuaria el bloqueo del auto-modo. Recuperable: los 52 archivos están versionados hasta `97ae158`.
- **15A-1b — Tokens heredados sin definición: HECHO (`76108cb`).** `frontend/src/styles/tokens.css` (borrado con el shell) definía 113 tokens que el CSS **vivo** referencia, pero su único importador era `App.css`, del shell; `main.tsx` nunca lo cargó, así que esas declaraciones ya eran inválidas en el navegador desde F1c/F3 (que reescribió 72 declaraciones a tokens que no llegaban al runtime). Nuevo `frontend/src/legacy/styles/tokensHeredados.css` con solo los 113 que faltan, importado en `main.tsx:27`. VarSense `variableNoDefinida` 336 → 0; gate 1075 → 739 con **0 nuevos**. Sin `sentinel-disable` ni exclusiones. **Revisión visual HECHA (15A-2)**: la landing renderiza en el taller con 147 hojas y 219 tokens `:root`; los únicos vacíos son `--sombraElevada` y `--sombraModal`, que `variables.css:73-74` declara `unset` a propósito (diseño plano). `--fondoBase #070707` → body `rgb(7,7,7)`, `--textoPrimario #e8e1c9` → texto `rgb(232,225,201)`, y el tema claro conmuta a `rgb(255,255,255)`. Cero colisiones con `variables.css`/`glory-core`: los 113 nombres son nuevos.
- Hecho en 15A-1 (commit `97ae158`): los 8 hallazgos `singleton-mutable-state` corregidos de raíz en `clients/desktop/src` (Set exportado → `readonly string[]`, `.has(` → `.includes(`), sin excepciones ni `sentinel-disable`. Backlog de falsos positivos y defectos de regla anotado en `Agente/planes/plan-calidad-sentinel-kamples-2026-10-13.md` (sesión 15A-1).
- admin/panel/ al recargar debería abrir la tab que estaba abierta no regresar al principio
- Revisar excepción cargo-audit RUSTSEC-2023-0071 (rsa): **vence 2026-11-12**. Ver `.cargo/audit.toml`. Quitarla si hay parche o sqlx-mysql sale del lock; si no, renovar con fecha nueva.

### ~~256A-2 — Reporte de verificación sync + documentación~~ ✅

**Problema:** No había forma estructurada para que el agente (o el usuario) verificara el estado completo del sistema de sincronización en una sola llamada.

| Subtarea | Descripción | Estado |
|---|---|---|
| 256A-2a | `generarReporteSync()` + `SyncReport` interface en `syncService.ts` | ✅ |
| 256A-2b | `__KAMPLES_SYNC_REPORT__` en `Window` interface (`global.d.ts`) | ✅ |
| 256A-2c | Exposición en `main.tsx` y `sync.tsx` | ✅ |
| 256A-2d | Documentación actualizada (doc sync + roadmap + archivado) | ✅ |

**Resultado:** `await window.__KAMPLES_SYNC_REPORT__()` retorna un objeto con auth, config, backend, tracking, uploadQueue, journal, circuitBreaker y auto-diagnósticos. Disponible en consola DevTools de ambas ventanas. Uso: `const r = await window.__KAMPLES_SYNC_REPORT__(); console.table(r.diagnosticos);`

### ~~256A-1 — Portar Desktop App (Tauri 2) al stack Rust~~ ✅

**Problema:** Los aliases `@` y `@app` apuntaban al tema WordPress que no existe en este proyecto. La SPA frontend ya tenía el código equivalente en `glory-core/` y `legacy/`.

**Plan ejecutado:** `Agente/planes/plan-portar-desktop-app-2026-06-25.md` → `Agente/planes/completados/`

| Subtarea | Descripción | Estado |
|---|---|---|
| 256A-1a | Arreglar aliases en `vite.config.ts` + `tsconfig.json` | ✅ |
| 256A-1b | Instalar dependencias faltantes (zustand, lucide-react, soundtouchjs) | ✅ |
| 256A-1c | Type check + stubs para Capacitor/Mezclador/Tauri | ✅ |
| 256A-1d | Smoke test Vite dev server (compila y resuelve imports) | ✅ |
| 256A-1e | Limpieza (`clients/Glory/` eliminado, TODO proxy actualizado) | ✅ |
| 256A-1f | Documentación actualizada (3 MDs) | ✅ |

**Resultado:** `npx tsc --noEmit` → 0 errores. `npx vite --port 1420` → servidor listo en 326ms. Todos los imports `@/core`, `@/core/hydration`, `@app/appIslands` resuelven correctamente.

- Remplazar el daw que hicimos, por https://github.com/andremichelle/opendaw, requiere revisar el daw a actual para ver como estaba integrado, y hacer un plan de las integraciones necesaria (como que un sample se pudiera arrastrar y soltar), el daw actual es muy pobre y malo, por eso vamos a cambiar a opendaw, tienes que hacer un fork en mi cuenta 1ndoryu. **Plan:** `Agente/planes/plan-opendaw-2026-04-25.md` — bloqueado en fase 0 (fork manual del usuario en `1ndoryu/opendaw`).

### Fase 19 — Despliegue
- Nota: un error pasado hizo que se borrara la base de datos, hay que reforzar para que no vuelva a suceder, el error probablemente fue causado al desplegar directamente o alguna otra razón, se debe tomar todo los medios necesarios para evitar la perdida de datos, tanto de la base de datos tanto como los archivos físicos que deben perdurar, esto debe tomarse muy en serio porque ya es un error que se cometió en el pasado.
- 174A-116 — Migración inicial automática + healthcheck (no hacerlo hasta mi confirmación)
- 174A-117 — Deploy via `coolify-manager-rs`(no hacerlo hasta mi confirmación)
- 174A-118 — Smoke test producción + rollback procedure (no hacerlo hasta mi confirmación)
