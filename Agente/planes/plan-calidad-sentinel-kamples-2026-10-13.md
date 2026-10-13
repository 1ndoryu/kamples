# Plan — Calidad Sentinel/VarSense en KAMPLES (13AA-1)

**Fecha:** 2026-10-13
**Origen:** workspace-manager reporta 2797 hallazgos Sentinel en KAMPLES. La usuaria eligió «Todos, por lotes» (AskUserQuestion).

## Objetivo

Resolver los hallazgos Sentinel de KAMPLES en su repo (AGENTS §6), por lotes, sin tocar el tablero. El estado de partida está en `workspace-manager` (escaneo forzado 2026-10-13, KAMPLES 2798 = 2797 Sentinel + 1 `sinCommit` obsoleto).

## Alcance

- `frontend/src/**` (CSS, TS, TSX) y configuración de calidad (`sentinel.config.json`, `varsense.config.json`).
- Hallazgos nuevos o en archivos tocados: se corrigen en el lote.

## No alcance

- Backend Rust y lógica de dominio no relacionada con los hallazgos.
- Deploy, producción y cambios en `config/settings.json`.
- Editar la copia vendorizada de `glory-css/` (se resuelve con alias en tokens).

## Fases

### F1 — Errores (252 hallazgos, severidad error)

| Lote | Regla | N | Acción | Estado |
|---|---|---|---|---|
| F1a | `limite-lineas-nivel-3` | 1 | Excluir `**/_generated/**` en `sentinel.config.json` (código generado por codegen; el default de Sentinel ya lo excluye, KAMPLES lo sobrescribe). | Hecho |
| F1b | `html-sin-origen-declarado` | 1 | Waiver `sentinel-disable-file` en `frontend/src/legacy/hooks/useArticuloDetalle.ts` (mock solo dev, HTML constante). `htmlProductoresPermitidos` no se lee desde la config (`core/analyzeDocument.ts` solo pasa `directoryExceptions` y `portableBoundaries`), así que la allowlist no es opción. | Hecho |
| F1c | `variableNoDefinida` | 250 | (1) Alias de compatibilidad en `frontend/src/styles/tokens.css` para los nombres en inglés que usa `glory-css/settings-panel.css` y `glory-core/index.css`, apuntando a tokens canónicos. (2) Sustituir en legacy los nombres en español no definidos por su token canónico (mapa de los investigadores, verificado contra los archivos de tokens). (3) `--colorBloque` (inline desde JS) y `--anchoPanelLateral` (`setProperty`) se documentan como excepción o llevan default. | Hecho |

**Riesgo visual de F1c:** estas variables hoy resuelven a valor inválido/inexistente, así que el render cambia al definirlas. Requiere revisión visual o registrar la limitación.

### F2 — Mecánica (warning/info/hint)

inline-style-prohibido 178, cssInlineReact 193, console-production 157, import-muerto 3, barras-decorativas 9, any-type-explicito 4, todo-prosa-sin-marcador 14, todo-pendiente 1, key-index-lista 10, objeto-mutable-exportado 5, token-unused 22, token-duplicado-cross-archivo 8, innerhtml-variable 1, singleton-mutable-state 1.

### F3 — Diseño (requiere decisión por caso)

dom-access-outside-platform 109, window-reference-outside-platform 141, limite-lineas 10 y limite-lineas-nivel-2 4, directorio-abarrotado 21, claseHuerfana 453, valorHardcoded 494, css-hardcoded-value 346, css-especificacion-diseno-local 128, css-adhoc-button-style 58, css-elemento-html-directo 48, cssInlineScript 58, modal-* ~60, componente-artesanal 4, usestate-excesivo 2, componente-sin-hook-glory 2, mixed-barrel-logic 2, button-clase-especifica 1, modal-con-titulo 8.

### F4 — Rust (`**/*.rs`, pendiente, rama de rescate)

Añadir `**/*.rs` a `includePatterns` de `sentinel.config.json` sube el análisis a 194 errores y rompe el gate `--cierre` (análisis con exit 1). Por eso el cambio vive solo en `rescate/sentinel-rs-kamples` (`f32fd05`, pusheada); `main` no lo incluye.

- expect-produccion-rs 109, ruta-post-sin-rate-limit 65, path-join-sin-canonicalize 18, god-object-rs 2.
- Decidir por lote antes de integrar la rama: corregir el código Rust, o excluir por regla con motivo documentado.

## Verificación por lote

1. `npm --prefix frontend run type-check`
2. `npm --prefix frontend run build`
3. `npm run quality:lock:check` y `npm run quality:doctor` en la raíz de KAMPLES
4. Relectura del tablero (`GET http://127.0.0.1:8787/api/consola/problemas`)
5. Cambios de comportamiento visual: registrar la limitación (E2E y verify-parity requieren backend en `:3001` y BD, no ejecutados en esta tarea).

Línea base: `Arranque gate "KAMPLES" --inicio` (hallazgos guardados en `workspace-manager/data/gate-inicio/KAMPLES.json`). Cierre: `Arranque gate "KAMPLES" --cierre` sin hallazgos nuevos.

## Decisiones

- **Alias en `tokens.css` en vez de editar `glory-css/` vendorizado:** no se toca código de terceros copiado; el cambio es reversible.
- **Waiver y no allowlist para el HTML mock:** la allowlist no está cableada desde config.
- **Excluir `_generated`:** el código lo regenera codegen; partirlo se perdería en el siguiente `npm run codegen`.

## Estado

- F1 (F1a, F1b, F1c): hecho. Gate `Arranque gate "KAMPLES" --cierre`: 0 nuevos, 254 resueltos (base 2797, ahora 2543).
- Siguiente paso: abrir F2 (mecánica) como lote en este plan.
- F2, F3: sin empezar.
- F4 (Rust, 194 errores): pendiente; el cambio de config vive en `rescate/sentinel-rs-kamples`, no en `main`.

## Definition of Done

- F1 cerrado con verificación §Verificación por lote.
- Hallazgos F2/F3 restantes listados con conteo actualizado en el tablero y enlazados aquí.
- Al cerrar cada fase: entrada en `Agente/completados/tareas-2026-10-13.md`.
