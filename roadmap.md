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

- Calidad Sentinel/VarSense KAMPLES (13AA-1): F1 hecho (0 nuevos, 2797 → 2543); F2 mecánica parcial; console-production hecho en escritorio (`3d55e63`) y en `useWebSocket.ts` (`2fcd15a`). Pendiente: console-production de `glory-core` (vendorizado, backlog), tokens y F3 de diseño con verificación visual; plan y backlog en [plan](Agente/planes/plan-calidad-sentinel-kamples-2026-10-13.md). F4 (Rust, 194 errores) pendiente: el cambio de config está en `rescate/sentinel-rs-kamples` (`f32fd05`), no en `main`.
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
