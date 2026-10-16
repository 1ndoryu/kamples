# Plan de Deploy Kamples → Coolify (desde cero)

> **Fecha:** 2026-06-30
> **Estado:** Pendiente de aprobación
> **Servicio:** `kamples`
> **VPS:** 66.94.100.241 (mismo que studio/nakomi)
> **Dominio test:** `samples.nakomi.studio`
> **Dominio prod:** `kamples.com` (después de verificar que todo funciona)

---

## Contexto

El proyecto `glory-rust-template` (Kamples) es un stack Rust + React que nunca se ha desplegado en producción vía Coolify. Existe documentación de deploy previa (`Agente/documentacion/deploy/secrets-2026-04-20.md`) y backups de BD del 2026-04-27, pero el deploy estaba bloqueado pendiente de aprobación.

El deploy anterior era PHP (WordPress). Hay que limpiarlo preservando los uploads de usuario.

**Archivos de referencia:**
- `docker-compose.yml` — stack completo (postgres, redis, app, web, backup sidecar)
- `Dockerfile` — backend Rust multi-stage con cargo-chef
- `frontend/Dockerfile` — frontend Vite + nginx multi-stage
- `.env.production.example` — plantilla de variables
- `Agente/documentacion/deploy/secrets-2026-04-20.md` — gestión de secrets y reglas anti-pérdida

---

## Fase 0 — Preparación local (sin tocar el servidor)

| Paso | Acción | Estado |
|---|---|---|
| 0.1 | Compilar `coolify-manager.exe` (`cargo build --release --target-dir target` en workspace coolify-manager-rs) | ⬜ |
| 0.2 | Generar secrets: `JWT_SECRET`, `POSTGRES_PASSWORD`, `WS_SECRET` con `openssl rand -base64 48` | ⬜ |
| 0.3 | Verificar `.sqlx/` actualizado (321 queries cacheadas — si hubo cambios en queries, regenerar con `cargo sqlx prepare`) | ⬜ |
| 0.4 | Verificar `openapi.json` refleja el schema actual | ⬜ |
| 0.5 | Verificar frontend compila: `cd frontend && npm run build` | ⬜ |
| 0.6 | Verificar backend compila: `cargo check` | ⬜ |

---

## Fase 1 — Limpiar deploy PHP anterior en el servidor

> **Objetivo:** Eliminar el stack PHP de kamples sin tocar los archivos de usuario.

| Paso | Acción | Riesgo |
|---|---|---|
| 1.1 | Verificar qué existe: `docker ps --filter "name=kamples"` y `ls -la /data/kamples/` | Diagnóstico |
| 1.2 | Backup de uploads: `./scripts/backup-uploads.sh pre-migracion` (verifica el tar antes de darlo por bueno) | Bajo |
| 1.3 | Backup de BD (si existe PostgreSQL de kamples): `./scripts/backup-postgres.sh pre-migracion` | Bajo |
| 1.4 | Parar stack PHP: `docker compose -f <ruta> down` (**SIN `-v`**) | Medio — verificar que no toca otros stacks |
| 1.5 | Eliminar contenedores/images PHP (NO volúmenes ni bind-mounts) | Bajo |
| 1.6 | Verificar que `/data/kamples/uploads/` sigue intacto | Diagnóstico |

---

## Fase 2 — Crear directorios y configurar en Coolify

| Paso | Acción |
|---|---|
| 2.1 | Crear directorios en host (si no existen): `mkdir -p /data/kamples/{uploads,db_dumps,app_logs}` + `chown -R 1000:1000 /data/kamples` |
| 2.2 | Crear aplicación "kamples" en Coolify UI: tipo Docker Compose, apuntar al repo Git |
| 2.3 | Configurar dominio `samples.nakomi.studio` en Coolify (Traefik genera TLS automáticamente) |
| 2.4 | Configurar variables de entorno en Coolify UI (ver Fase 3) |

---

## Fase 3 — Variables de entorno

### Requeridas (sin estas no arranca)

| Variable | Valor | Generación |
|---|---|---|
| `POSTGRES_DB` | `kamples` | Fijo |
| `POSTGRES_USER` | `kamples` | Fijo |
| `POSTGRES_PASSWORD` | `<random>` | `openssl rand -base64 32` |
| `JWT_SECRET` | `<random>` | `openssl rand -base64 48` |
| `WS_SECRET` | `<random>` | `openssl rand -base64 48` |
| `PUBLIC_BASE_URL` | `https://samples.nakomi.studio` | Dominio test |
| `WS_PUBLIC_URL` | `wss://samples.nakomi.studio/api/ws` | Dominio test |

### Opcionales (deshabilitan canal si faltan)

| Variable | Servicio | Generación |
|---|---|---|
| `GOOGLE_CLIENT_IDS` | Google Auth | CSV de client_ids de Google Console |
| `STRIPE_SECRET_KEY` | Stripe | Dashboard Stripe |
| `STRIPE_PUBLISHABLE_KEY` | Stripe | Dashboard Stripe |
| `STRIPE_WEBHOOK_SECRET` | Stripe | Dashboard Stripe → Webhooks |
| `SMTP_HOST` | Email | Proveedor SMTP |
| `SMTP_PORT` | Email | `587` |
| `SMTP_USER` | Email | Proveedor SMTP |
| `SMTP_PASSWORD` | Email | Proveedor SMTP |
| `SMTP_FROM_EMAIL` | Email | `noreply@kamples.com` |
| `SMTP_FROM_NAME` | Email | `Kamples` |
| `VAPID_PUBLIC_KEY` | Web Push | `npx web-push generate-vapid-keys` |
| `VAPID_PRIVATE_KEY` | Web Push | `npx web-push generate-vapid-keys` |
| `VAPID_SUBJECT` | Web Push | `mailto:admin@kamples.com` |

### Avanzadas (defaults razonables)

| Variable | Default | Nota |
|---|---|---|
| `RUST_LOG` | `glory_backend=info,tower_http=info,sqlx=warn` | Ajustar para debug |
| `LOG_FORMAT` | `json` | `text` para desarrollo |
| `DB_MAX_CONNECTIONS` | `20` | Ajustar según VPS |
| `DB_MIN_CONNECTIONS` | `2` | — |
| `STORAGE_BACKEND` | `local` | `s3` requiere feature flag |
| `BACKUP_INTERVAL_SECONDS` | `21600` | 6 horas |
| `BACKUP_RETENTION_DAYS` | `14` | — |

---

## Fase 4 — Primer deploy

| Paso | Comando | Tiempo |
|---|---|---|
| 4.1 | `cm deploy --name kamples --update --skip-backup` | ~8-12 min |
| 4.2 | `cm health --name kamples` | ~30s |
| 4.3 | `cm logs --name kamples` | Revisar |
| 4.4 | Verificar migraciones automáticas (SQLx corre `migrations/` al boot) | — |

> **Nota:** Si devuelve 503 durante el build, es **normal** — Rust toma tiempo. Esperar.

---

## Fase 5 — Verificación post-deploy

| Check | Método | Estado |
|---|---|---|
| Backend health | `GET https://samples.nakomi.studio/api/health` → 200 | ⬜ |
| Frontend carga | Abrir en browser | ⬜ |
| Logs sin errores | `cm logs --name kamples` | ⬜ |
| Migraciones aplicadas | `cm exec --name kamples --target app --command "ls /app/migrations"` | ⬜ |
| Uploads preservados | `cm exec --name kamples --target app --command "ls /data/uploads/"` | ⬜ |
| Auth funcional | Probar login/registro | ⬜ |
| WebSocket | Probar conexión WS | ⬜ |

---

## Fase 6 — Rollback (si algo falla)

| Escenario | Acción | Tiempo |
|---|---|---|
| Build Docker falla | Revisar logs build, corregir Dockerfile, `cm deploy --name kamples --update` | — |
| Health check falla tras deploy | `cm redeploy --name kamples` → `cm health` | ~10 min |
| Health sigue fallando tras redeploy | Restaurar backup: ver abajo | ~5 min |
| Migraciones rompen datos | Restore de dump (ver abajo) | ~3 min |
| Frontend no carga | Verificar nginx.conf, `cm logs --name kamples --target web` | — |
| 503 durante build | **Normal** — Rust toma 8-12 min, esperar | — |
| Stack no arranca (error compose) | Revisar `cm logs --name kamples`, corregir compose, redeploy | — |
| **Catástrofe total** | Ver "Procedimiento de escape" abajo | ~15 min |

### Restore de base de datos

**NO se restaura con `pg_restore --clean --if-exists` a mano.** Esa línea
estuvo aquí y es exactamente lo que dejó una base vacía: si el dump está vacío
o truncado, `--clean` borra la base actual y luego no repone nada. Usa el
script, que verifica el dump antes de tocar nada, saca copia previa verificada
y restaura en una sola transacción:

```bash
# Ver qué hay (solo lectura)
cm exec --name kamples --target postgres -- ls -lt /dumps/

# Simulación: verifica el dump y dice qué haría, sin tocar nada
PG_EXEC='cm exec --name kamples --target postgres --' \
    ./scripts/restore-postgres.sh /data/kamples/db_dumps/<archivo>.dump

# Restaurar de verdad
PG_EXEC='cm exec --name kamples --target postgres --' \
    ./scripts/restore-postgres.sh /data/kamples/db_dumps/<archivo>.dump --aplicar
```

Luego reinicia la app para reconectar: `cm restart --name kamples`.

Detalle y garantías: `Agente/documentacion/deploy/proteccion-datos-2026-10-16.md`.

### Procedimiento de escape (catástrofe total)
Si nada funciona y hay que revertir todo:
1. `cm stop --name kamples` (o `docker compose down` en el directorio del stack)
2. Verificar que studio y nakomi siguen arriba: `cm health --name studio && cm health --name nakomi`
3. Restaurar directorio `/data/kamples/uploads` desde backup tar.gz
4. Dejar constancia del fallo en `Agente/lecciones/` para no repetirlo

---

## Aislamiento de otros despliegues — Reglas de seguridad

> **Principio absoluto:** studio (`nakomi.studio`) y nakomi (`task.nakomi.studio`) están en producción. Cualquier operación que los afecte es un incidente.

### Qué NUNCA hacer

| Operación prohibida | Por qué | Consecuencia |
|---|---|---|
| `docker compose down -v` (en cualquier stack) | `-v` borra TODOS los volúmenes del compose, incluyendo los de otros stacks si comparten red | Pérdida de datos irreversible |
| `restart --all` en Coolify | Reinicia TODOS los servicios — Rust no se recupera sin rebuild (incidente 2026-05-11) | studio + nakomi caídos |
| `docker system prune -a` | Borra images, containers, networks y volumes de TODOS los proyectos | Todos los stacks caídos |
| `docker volume prune` | Puede borrar volúmenes huérfanos de otros stacks | Datos perdidos |
| Operar sobre UUID/nombre que no sea `kamples` | Confundir `kamples` con `studio` o `nakomi` | Stack equivocado cae |
| Modificar Traefik/red de Coolify | Afecta el routing de TODOS los dominios | Todos los sitios inaccesibles |
| SSH + `docker compose up` directo | Coolify puede sobreesbrir el compose, causando conflicto | Estado inconsistente |

### Qué SÍ hacer (checklist pre-operación)

Antes de CADA operación en el servidor:

```
□ Confirmar que estoy operando sobre "kamples" (no studio, no nakomi)
□ Verificar que studio sigue arriba: cm health --name studio
□ Verificar que nakomi sigue arriba: cm health --name nakomi
□ Si la operación es destructiva: backup primero
□ Si la operación toca Docker: usar solo comandos de coolify-manager (nunca docker directo)
□ Si hay duda: parar y preguntar
```

### Aislamiento técnico (ya garantizado por Coolify)

| Capa | Mecanismo | ¿Kamples afecta a otros? |
|---|---|---|
| **Red Docker** | Cada stack tiene su propia red aislada | ❌ No |
| **Volúmenes** | Nombres únicos: `kamples_postgres_data`, `kamples_redis_data` | ❌ No |
| **Bind-mounts** | Rutas exclusivas: `/data/kamples/` | ❌ No |
| **Contenedores** | Prefijo por stack en Coolify | ❌ No |
| **Dominios** | Traefik routea por FQDN — `samples.nakomi.studio` no toca `nakomi.studio` | ❌ No |
| **Process restart** | `cm restart --name kamples` solo reinicia contenedores de kamples | ❌ No |

> **Único punto de riesgo compartido:** los recursos del VPS (CPU, RAM, disco). Si kamples consume demasiado, los otros stacks se degradan. Mitigación: `deploy.resources.limits` ya están en docker-compose.yml (postgres: 2GB RAM).

### Checks post-operación obligatorios

Después de CADA operación que toque el servidor:

```powershell
# 1. Verificar que otros stacks siguen vivos
& $cm health --name studio
& $cm health --name nakomi

# 2. Verificar que kamples está en el estado esperado
& $cm health --name kamples
& $cm logs --name kamples --tail 20

# 3. Si algo se ve raro: NO seguir operando, diagnosticar primero
```

---

## Riesgos y mitigaciones (matriz completa)

| # | Riesgo | Probabilidad | Impacto | Mitigación | Responsable |
|---|---|---|---|---|---|
| R1 | Borrar uploads del PHP anterior | Media | Alto | Backup tar.gz **antes** de limpiar + bind-mount preservado | Agente |
| R2 | Colisión DNS `postgres` con otros stacks | Baja | Alto | Si hay colisión, usar `postgres-<uuid>` en DATABASE_URL | Agente |
| R3 | `restart --all` afecta studio/nakomi | Baja | Crítico | **NUNCA usar** — siempre operar solo sobre `kamples` | Agente |
| R4 | Pérdida de BD | Baja | Crítico | Sidecar `postgres-backup` (cada 6h) + backup pre-deploy manual | Agente |
| R5 | `.sqlx/` desactualizado → build falla | Baja | Medio | Verificar con `cargo sqlx prepare` antes del build | Agente |
| R6 | Consumo excesivo de recursos VPS | Media | Medio | `deploy.resources.limits` en compose (postgres: 2GB) | Agente |
| R7 | Dominio `samples.nakomi.studio` conflicto con nakomi | Baja | Medio | Son FQDNs distintos — Traefik los routea independientemente | Agente |
| R8 | Migraciones SQL rompen schema existente | Baja | Alto | Backup pre-deploy + restore si falla | Agente |
| R9 | Build Docker toma demasiado → timeout | Media | Bajo | Esperar 8-12 min — es normal para Rust | Agente |
| R10 | Error en compose → stack no arranca | Baja | Medio | Revisar logs, corregir compose, redeploy | Agente |
| R11 | Rewrite `/wp-json/*` rompe endpoints existentes | Baja | Medio | Los alias son adicionales — no modifican `/api/*` existentes | Agente |
| R12 | Tauri CSP bloquea `samples.nakomi.studio` | Baja | Bajo | Agregar dominio al CSP antes de probar | Agente |
| R13 | Coolify API 422/500 al crear servicio | Baja | Medio | Documentado en SKILL.md — usar `--skip-compose-sync` si ocurre | Agente |

---

## Reglas absolutas (anti-pérdida de datos)

1. **NUNCA** `docker compose down -v` — borra `kamples_postgres_data`
2. **NUNCA** renombrar volúmenes ya creados
3. **Antes de deploy/migración:** backup manual con `scripts/backup-postgres.sh pre-deploy`
4. **Bind-mount** `/data/kamples/uploads` → `/data/uploads` — NO mover sin migrar archivos
5. **Sidecar** `postgres-backup` cada 6h, rotación 14 días — NO sustituye backup off-site
6. **`restart: unless-stopped`** — no reinicia si se detuvo manualmente
7. **Toda operación en servidor:** verificar studio + nakomi después
8. **Si hay duda:** parar y preguntar al usuario
9. **Migraciones:** solo con `scripts/migrar.sh` (guardia + copia verificada + una transacción). El `psql` a mano no para ante un error.
10. **Restaurar:** solo con `scripts/restore-postgres.sh`. **NUNCA** `pg_restore --clean` directo sobre un dump sin verificar.
11. **Uploads:** copia con `scripts/backup-uploads.sh`. La base de datos NO contiene los archivos físicos.

---

## Secuencia de ejecución

```
Fase 0 (local) → Fase 1 (limpiar PHP) → Fase 2 (Coolify UI) → Fase 3 (envs) → Fase 4 (deploy) → Fase 5 (verificar)
```

---

## Fase 7 — Verificar app Tauri desktop (post-deploy)

> Después del deploy, verificar que la app Tauri local (`npm run tauri:dev` desde raíz o `tauri dev` desde `clients/desktop/`) funciona correctamente conectándose al backend local.

### Configuración actual de URLs en Tauri

| Contexto | URL | Archivo |
|---|---|---|
| **Dev (default)** | `http://localhost:3000` | `vite.config.ts` vía `KAMPLES_API_TARGET` |
| **Producción** | `https://kamples.com/wp-json` | `apiDesktopAdapter.ts` → `SERVIDOR_PROD` |
| **Producción API** | `https://api.kamples.com` | `tauri.conf.json` (CSP + updater) |
| **Backend selector** | `VITE_KAMPLES_BACKEND=rust` | `apiDesktopAdapter.ts` — `'rust'` activa adaptador WP→Rust |
| **Proxy rewrite** | `/wp-json/kamples/v1/*` → `/api/*` | `wpJsonRustAdapter.ts` |

### Verificaciones

| Check | Comando/Método | Estado |
|---|---|---|
| Backend local arrancado | `cargo run` o `npm run dev` (backend en `localhost:3000`) | ⬜ |
| Tauri dev arranca | `npm run tauri:dev` desde raíz (usa `scripts/launch-tauri.mjs`) | ⬜ |
| Login funciona | Probar login en la ventana Tauri | ⬜ |
| Sync funciona | Abrir panel sync, verificar conexión al backend | ⬜ |
| Google Auth | Probar flujo OAuth Google (callback loopback) | ⬜ |
| Sin errores en consola | Revisar DevTools de la ventana Tauri | ⬜ |

### Notas importantes

- **`npm run tauri:dev`** orquesta: limpia `CARGO_TARGET_DIR`, crea junctions en `public/`, espera al backend en puerto 3000, lanza `tauri dev`.
- El selector `VITE_KAMPLES_BACKEND=rust` activa el adaptador que reescribe URLs WordPress → Rust (`wpJsonRustAdapter.ts`).
- En dev, el proxy de Vite redirige `/api` → `http://localhost:3000` (el backend local).
- En producción, `apiDesktopAdapter.ts` usa `https://kamples.com/wp-json` — **esto apunta al deploy PHP legacy**. Si el deploy Rust usa `kamples.com`, hay que actualizar `SERVIDOR_PROD` a `https://kamples.com/api` o asegurar que el rewrite `/wp-json/*` → `/api/*` funcione en el backend Rust.
- El updater de Tauri apunta a `https://api.kamples.com/api/app/updater/...` — esto requiere que el backend exponga ese endpoint.

### Decisión: Rewrite en backend (Opción C)

**El backend Rust mapea `/wp-json/*` → `/api/*`**. Así los clientes Tauri (y cualquier otro consumidor legacy) no necesitan cambios. El adapter `wpJsonRustAdapter.ts` ya hace rewrite client-side, pero el server-side es más robusto.

**Archivos a modificar en el backend (pre-deploy):**

| Archivo | Cambio |
|---|---|
| `src/main.rs` o router Axum | Agregar alias de rutas: `.route("/wp-json/kamples/v1/*path", get(api_handler).post(api_handler))` que redirijan a los handlers existentes de `/api/*` |
| `tauri.conf.json` (CSP) | Agregar `samples.nakomi.studio` a `connect-src` para el test |

**Archivos Tauri que NO se tocan** (ya funcionan con el rewrite):
- `apiDesktopAdapter.ts` — `SERVIDOR_PROD = 'https://kamples.com/wp-json'` se mantiene
- `sync.tsx` — `serverUrl: 'https://kamples.com/wp-json'` se mantiene
- `googleAuthMobileService.ts` — URLs `/wp-json/kamples/v1/auth/google/*` se mantienen
- `wpJsonRustAdapter.ts` — rewrite client-side se mantiene (doble capa de compatibilidad)

**Endpoints WP→Rust que el backend debe mapear** (basado en `wpJsonRustAdapter.ts`):
```
/wp-json/kamples/v1/sync/changelog   → /api/sync/changelog
/wp-json/kamples/v1/me/sync/delta    → /api/me/sync/delta
/wp-json/kamples/v1/me/sync/colecciones → /api/me/sync/colecciones
/wp-json/kamples/v1/auth/google/*    → /api/auth/google/*
```

> **Nota:** No es necesario mapear TODOS los endpoints WP legacy — solo los que la app Tauri usa. Los demás pueden ir llegando según se necesiten.

---

## Pendientes

- [ ] Compilar coolify-manager.exe
- [ ] Generar secrets
- [ ] Verificar .sqlx/ y openapi.json
- [ ] Limpiar deploy PHP anterior
- [ ] Crear servicio en Coolify
- [ ] Configurar variables de entorno
- [ ] Primer deploy
- [ ] Verificación completa
- [ ] Verificar app Tauri local (`npm run tauri:dev`)
- [ ] Implementar rewrite `/wp-json/*` → `/api/*` en backend Rust (antes del deploy)
- [ ] Agregar `samples.nakomi.studio` al CSP de `tauri.conf.json`
- [ ] Configurar dominio prod (kamples.com)
- [ ] Configurar backup off-site (rclone/restic → S3/B2)
