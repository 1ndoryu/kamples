# Plan — Reemplazo del DAW interno por opendaw

> **Tarea:** roadmap pendiente — "Remplazar el daw que hicimos, por https://github.com/andremichelle/opendaw"
> **Fecha:** 2026-04-25
> **Origen:** `frontend/src/mezclador/` + `frontend/src/features/mezclador/` (DAW propio actual)
> **Destino:** Integracion de opendaw como motor del DAW.
> **Repo upstream:** https://github.com/andremichelle/opendaw
> **Fork requerido:** github.com/1ndoryu/opendaw (accion manual del usuario, ver fase 0).

## Contexto

El DAW actual (`Mezclador`) es una implementacion propia con limitaciones:
- 2 carpetas paralelas (`mezclador/` y `features/mezclador/`) — el origen del duplicado y cual se usa hay que resolverlo en la fase 1.
- Funcionalidad pobre comparada con opendaw (sintesis, MIDI, mixer pro, etc.).
- Mantenerlo costaria mas que adoptar opendaw.

**Objetivo:** Que un usuario pueda arrastrar un sample desde la biblioteca al DAW y empezar a producir con un editor pro.

## Inventario de lo que se va a tirar/migrar

Lo que hay que auditar antes de empezar:
1. `frontend/src/mezclador/` — types, stores zustand, services, hooks, utils, componentes.
2. `frontend/src/features/mezclador/` — duplicado o version anterior. Revisar diferencias.
3. `frontend/src/app/styles/legacyUi.css` — dependencias UI legacy del Mezclador.
4. `frontend/src/app/stores/panelLateralStore.ts` — modo `'mezclador'` que abre el panel.
5. `frontend/src/app/stores/crearModalStore.ts` — flujo "abrir modal con archivo precargado" que el Mezclador dispara.
6. Endpoints backend que sirven al DAW: revisar `/api/samples` agrupado por tipo (`apiExplorador.ts`).
7. Persistencia de proyectos del DAW — si existe en backend, decidir si se migra o se descarta.

## Fases

### Fase 0 — Fork y exploracion del upstream — **CERRADA (2026-10-16)**

**El bloqueante que figuraba aquí era falso.** Decía «el agente NO tiene
credenciales de GitHub para crear el fork»; `gh` está autenticado como
`1ndoryu` con scope `repo`, y el fork ya existe:
**https://github.com/1ndoryu/openDAW** (verificado: `isFork: true`, padre
`andremichelle/openDAW`, rama `main`).

**BLOQUEO REAL, y no es técnico: la licencia.** opendaw es
**AGPL-3.0-or-later** (`LICENSE`, `package.json:5`). La AGPL es copyleft e
incluye el **uso por red** (§13): un servicio en red que incorpore el código
cuenta como distribución. Incrustarlo en KAMPLES —que es cerrado y cobra—
obliga a liberar el código correspondiente bajo AGPL **o** a comprar la
licencia comercial del autor. **Esto lo decide la usuaria; ninguna fase
posterior tiene sentido hasta entonces.**

Exploración hecha sobre el clon en `C:\tmp\opendaw\openDAW` (commit `98efab8`):

- 0.1 **Estructura**: monorepo npm workspaces + turbo (build) + lerna (publish).
  `packages/studio/*` es el dominio del DAW, `packages/lib/*` son librerías
  genéricas, `packages/app/studio` es la app principal
  (`@opendaw/app-studio`, `private: true`, no publicada). Node ≥23, TS ^5.9.
- 0.2 **API pública**: **no hay forma de embeber la UI.** Cero coincidencias de
  `customElements.define`, de un `mount()` exportado y de API de iframe;
  `boot()` (`packages/app/studio/src/boot.ts:45`) es interno de la app privada.
  Lo consumible es `@opendaw/studio-sdk` (meta-instalador) y, headless,
  `@opendaw/studio-core` (Project/ProjectApi/Engine/Mixer). **La UI propia no se
  reutiliza: habría que construirla contra el core, o hacer fork y merge.**
- 0.3 **Licencia**: AGPL-3.0-or-later (arriba). Algunos paquetes son LGPL-3.0
  (icons, markdown, scrollbars, manual) o ISC.
- 0.4 **Drag & drop**: viene de serie y es lo que la usuaria pedía.
  `DragAndDrop.ts:22-24` detecta `"Files"`; `TimelineDragAndDrop.ts:59-66` hace
  `sampleService.importFiles(data.files)`; también en `SampleSelector.ts:147`,
  `ChopTrigger.ts:54` y `ResourceBrowser.tsx:187`.
- **Condicionante técnico añadido**: el motor exige **COOP/COEP +
  SharedArrayBuffer** (`packages/studio/sdk/README.md:23`). Servir la app de
  KAMPLES con esas cabeceras rompe la mayoría de embeds de terceros.
- **Sin medir**: el tamaño del build (no hay `dist/` en el repo; compilar
  requiere Rust ≥1.82 nightly + binaryen).

### Fase 1 — Auditoria del Mezclador actual (1 sesion)
- 1.1 Diff entre `frontend/src/mezclador/` y `frontend/src/features/mezclador/`. Decidir cual es la version "buena" y borrar la otra.
- 1.2 Listar funcionalidades del Mezclador actual (tracks, sampler, transport, etc.) para mapear que cubre opendaw y que no.
- 1.3 Listar estados zustand y servicios que persisten datos del DAW al backend, si hay.
- 1.4 Documentar resultados en `Agente/documentacion/daw/auditoria-mezclador-actual-{fecha}.md`.

### Fase 2 — Estrategia de integracion (1 sesion)
Decidir entre:
- (A) **Embed:** opendaw se monta como sub-componente React en una ruta `/daw` o panel. Comunicacion via postMessage o ref.
- (B) **iframe:** opendaw corre en un iframe (su propia app) y kamples envia/recibe mensajes (drag-drop -> postMessage).
- (C) **Fork-and-merge:** copiar codigo de opendaw dentro de glory-rust-template y mergear.

Recomendacion preliminar (sin haber leido opendaw): (B) iframe — preserva el upstream, facilita merges, encapsula complejidad. (A) si opendaw exporta componentes React puros.

Documentar en `Agente/documentacion/daw/estrategia-opendaw-{fecha}.md`.

### Fase 3 — Integracion de drag-drop con biblioteca (2-3 sesiones)
- 3.1 Sample card en biblioteca expone `onDragStart` con payload tipado `{ sampleId, sampleUrl, sampleHash, duracion, bpm, key }`.
- 3.2 Receptor en opendaw (a traves de postMessage o drop handler) crea una pista nueva con el sample.
- 3.3 Backend: garantizar que `ruta_preview`/`ruta_optimizada` son CORS-friendly para que el motor de audio de opendaw pueda fetchear.
- 3.4 Tests manuales: arrastrar 5 samples diferentes, verificar reproduccion y sync.

### Fase 4 — Persistencia de proyectos (si aplica)
- 4.1 Definir formato de proyecto opendaw (probablemente JSON).
- 4.2 Backend: tabla `proyectos_daw { id, usuario_id, nombre, datos JSONB, ... }` o reutilizar existente si la auditoria revela una.
- 4.3 Endpoints CRUD `/api/me/proyectos-daw`.
- 4.4 UI de "Guardar proyecto" / "Abrir proyecto" en kamples shell.

### Fase 5 — Cleanup del Mezclador legacy
- 5.1 Eliminar `frontend/src/mezclador/` y `frontend/src/features/mezclador/` (lo que sobre).
- 5.2 Eliminar `legacyUi.css`, types compatibilidad, modos zustand obsoletos.
- 5.3 Actualizar landing (`landing.seccion.daw.subtitulo`) si cambia el copy.

### Fase 6 — Cierre
- 6.1 Mover este plan a `Agente/planes/completados/`.
- 6.2 Documentar arquitectura final en `Agente/documentacion/daw/arquitectura-opendaw-{fecha}.md`.

## Riesgos y dudas

- Licencia de opendaw: si es GPL, kamples (cerrado/comercial) podria tener conflicto. Revisar LICENSE en fase 0.
- Performance: opendaw es una app web pesada. Cargarla solo cuando el usuario abre el panel del DAW (lazy import).
- Compatibilidad de stack: opendaw probablemente usa Vite + TypeScript. Debe encajar con nuestro frontend Vite.
- Datos legacy: si hay proyectos guardados con el formato del Mezclador actual, ¿se migran? Evaluar en fase 1.

## Estado actual (2026-04-25)
- Plan inicial escrito.
- ~~BLOQUEANTE FASE 0: el usuario debe hacer el fork manualmente~~ — resuelto el 2026-10-16: el fork existe en `1ndoryu/openDAW`.
- **BLOQUEANTE NUEVO: la licencia AGPL-3.0-or-later de opendaw.** Decisión de la usuaria. Sin ella no se avanza a la fase 1.
- Agente puede avanzar en investigacion del upstream (clonar, leer codigo) sin bloqueante.
- No hay deadline; se prioriza despues de las tareas mas chicas del roadmap.

## Bitacora
- 2026-04-25: plan creado. Sin bloqueante de aclaracion del usuario, pero fork pendiente como accion manual.
- 2026-10-16: fork creado por el agente (`1ndoryu/openDAW`); el bloqueante declarado era falso. Fase 0.1–0.4 completada sobre el clon. **Aparece el bloqueo real: AGPL-3.0-or-later** — incompatible con un producto cerrado de pago salvo licencia comercial o liberación del código. Fase 0 cerrada a la espera de esa decisión.
