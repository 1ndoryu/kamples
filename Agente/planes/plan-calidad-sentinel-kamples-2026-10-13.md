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
6. **Datos de prueba para verificar (pedido de la usuaria, 2026-10-13):** para cada zona tocada, probar con datos reales en el entorno local: crear una cuenta admin de prueba, subir contenido de prueba (audio, imágenes y demás según la zona) y recorrer los flujos afectados. Solo en host local (`localhost`), con valores de prueba generados por el agente; nunca credenciales reales ni de producción; nunca `config/settings.json` en commits.
7. **Sin cambios visuales:** la UI de KAMPLES ya se ve bien. Se permiten ajustes menores de coherencia visual, pero el objetivo es que todo sea coherente como se espera, no rediseñar. Comparar con la huella de render de las rutas antes y después de cada lote.

Línea base: `Arranque gate "KAMPLES" --inicio` (hallazgos guardados en `workspace-manager/data/gate-inicio/KAMPLES.json`). Cierre: `Arranque gate "KAMPLES" --cierre` sin hallazgos nuevos.

## Decisiones

- **Alias en `tokens.css` en vez de editar `glory-css/` vendorizado:** no se toca código de terceros copiado; el cambio es reversible.
- **Waiver y no allowlist para el HTML mock:** la allowlist no está cableada desde config.
- **Excluir `_generated`:** el código lo regenera codegen; partirlo se perdería en el siguiente `npm run codegen`.

## Estado

- F1 (F1a, F1b, F1c): hecho. Gate `Arranque gate "KAMPLES" --cierre`: 0 nuevos, 254 resueltos (base 2797, ahora 2543).
- F2 (mecánica): lotes hechos en `ffb669f`, `e695539`, `1cdd550`. Tablero 2026-10-13 13:04Z: KAMPLES 2473 (base 2797).
- F2 restante (fuera de F3 visual):
  - `console-production`: hecho en `3d55e63` (`clients/desktop`, 176 llamadas → `logSync`, tsc 0 errores) y `2fcd15a` (`useWebSocket.ts`, comentario JSDoc). Queda `frontend/src/glory-core` 23 (vendorizado, no se edita: va al backlog).
  - **Decidido (logger):** `clients/desktop` usa `logSync` de `src/services/syncLogger.ts`, que guarda en el buffer de diagnóstico y además imprime en consola (`agregarAlBuffer`), así que es reemplazo directo sin perder salida. Mapeo: `console.log`→`logSync.info`, `info`→`info`, `warn`→`warn`, `error`→`error`; módulo = nombre del archivo sin extensión. `frontend/src/legacy` usa `crearLogger` (en producción solo emite errores: los `info/debug` dejan de salir en producción; es el diseño del logger, y los errores siguen visibles).
  - Sin `node_modules` en `clients/desktop`: `npm ci` previo al baseline de `type-check` (hecho, 142 paquetes; `tsc --noEmit` = 0 errores).
  - Baseline `frontend` `type-check`: 3 errores TS2591 en `frontend/src/legacy/hooks/useAuth.ts` (`require` sin `@types/node`), preexistentes y no causados por estos cambios. Pendiente de explicar, no de corregir en F2.
  - `token-unused` 22, `token-duplicado-cross-archivo` 8: tokens de `frontend/src/styles/tokens.css`; cambian CSS, verificar visualmente.
  - `barras-decorativas` 3, `any-type-explicito` 1, `todo-prosa-sin-marcador` 1, `singleton-mutable-state` 1: pequeños, sin decisión de diseño.
- F3 (diseño y CSS, requiere verificación visual; sin visuales nuevos): `valorHardcoded` 494, `claseHuerfana` 453, `css-hardcoded-value` 346, `cssInlineReact` 193, `inline-style-prohibido` 178, `window-reference-outside-platform` 141, `css-especificacion-diseno-local` 128, `dom-access-outside-platform` 109, `css-adhoc-button-style` 58, `cssInlineScript` 58, `css-elemento-html-directo` 48, `modal-*` 60, `directorio-abarrotado` 21, `limite-lineas` 9+3, `modal-con-titulo` 8, `componente-artesanal` 4, `usestate-excesivo` 2, `componente-sin-hook-glory` 2, `mixed-barrel-logic` 2, `button-clase-especifica` 1.
  - Por raíz: `frontend` 1111, `[varsense] frontend` 1228 (solapan con las de CSS), `clients` 134.
  - Lote F3 font-size (`c092ae5`): 172 declaraciones → tokens `--fuente*` (sin cambio de valor resuelto).
  - Lote F3 colores (`c8540f7`): 72 declaraciones `color/background/border-color` en 14 archivos → `--colorBlancoPuro`…`--colorFondoPanelAzul` (tokens.css :root, invariantes al tema).
  - Lote F4 colores (`040c55c`): 33 declaraciones hex (`#2a2a4a`, `#1e1e38`, `#16162a`, `#ef4444`…) → tokens `--colorFondoAzulNoche`…`--colorVerdeTerminal`.
  - Pendiente tras F4: `border:` shorthand con hex (`pianoRoll.css`, no lo cubre la regla), rgba() de fondos, `border-radius` 4–28px, `font-size` 12px/14px/11px/`0.92rem` restantes, y `var(--error, #e55353)` (fallback literal).
- F4 (Rust, 194 errores): pendiente; el cambio de config vive en `rescate/sentinel-rs-kamples`, no en `main`.
- **Sesión 13AA-1 (cierre parcial):**
  - Puertos Rust (arquitectura, pedido de la usuaria): `src/services/admin_processes.rs` fallback `PORT` 3000 → 3001; `src/handlers/payments.rs` y `connect.rs` `site_base_url` fallback 3000 → 5183 (sitio público de dev = frontend Vite, no backend); test `sample_urls_fall_back_to_sample_id_when_slug_is_missing` actualizado a 5183.
  - Verificación: `cargo check --tests` OK; `cargo test --lib sample_urls` 1/1 OK (`CARGO_TARGET_DIR=C:/tmp/glory-target/kamples-13aa`).
  - CSS: `--colorRojoSuave` eliminado de `tokens.css` (token-unused, sin usos en el repo; sin cambio visual).
  - Gate `Arranque gate "KAMPLES" --cierre`: 0 nuevos, 827 resueltos, **1970 abiertos** (base 2797).
  - Tablero (`escaneadoEn` 2026-10-13T14:00Z, KAMPLES total 1971) por regla: claseHuerfana 453, css-hardcoded-value 245, valorHardcoded 226, cssInlineReact 193, inline-style-prohibido 178, window-reference-outside-platform 141, css-especificacion-diseno-local 128, dom-access-outside-platform 109, css-adhoc-button-style 58, cssInlineScript 58, css-elemento-html-directo 48, modal-* 50, console-production 23 (todas en `glory-core`, vendorizado), directorio-abarrotado 21, limite-lineas 9+3, modal-con-titulo 8, componente-artesanal 4, otras ≤3.
  - **No resuelto en esta sesión y por qué:** F3 (≈1.9k) requiere decisión por caso y verificación visual (p. ej. `border-radius` 6–14px no tiene token exacto: cambiarlo sí altera el render). `limite-lineas` y `directorio-abarrotado` exigen dividir o mover archivos (refactor estructural). `usestate-excesivo`, `componente-sin-hook-glory` y `mixed-barrel-logic` son refactors de comportamiento. `console-production` (glory-core) es vendorizado: no se edita.
  - **Bloqueo de verificación funcional:** navegador y backend (cuenta admin, subida de audio/imágenes) siguen sin credenciales de PostgreSQL de prueba. Propuesta: base `kamples_verif` (no `glory_db`), credenciales que aporte la usuaria.
- **Sesión 13AA-3 (claseHuerfana CSS):**
  - Lote `4c7ba01`: 268 reglas en 37 archivos de `frontend/src/legacy/styles` quitadas. Criterio: todos los selectores de la regla contienen una clase huérfana (nombre ausente de todo `frontend/src` como literal y sin construcción dinámica por prefijo/sufijo; 199 nombres únicos; cero referencias fuera de `frontend/src`). Script: scratchpad de la sesión, no en el repo.
  - Verificación: `npx vite build` exit 0 (`C:/tmp/kamples-dist-13aa3`); gate `--cierre` 0 nuevos, 1050 resueltos, 1747 abiertos (base 2797); diff solo CSS (1771 líneas borradas, 0 añadidas).
  - Sin cambio visual esperado: ningún elemento del DOM puede coincidir con estas reglas. **No verificado en navegador** (bloqueo de credenciales); verificación estática.
  - Riesgo conocido: el guard de construcción dinámica es heurístico (tallo + ≤4 caracteres antes de `${` o `"+`; sufijo tras `}`). Un nombre compuesto con separador mayor no se detectaría; la búsqueda de composición en `frontend/src` no encontró ninguno.
  - Decidido: no se borran reglas con `:not(.x)` ni selectores mixtos; la única regla con clase huérfana en `:not` (`.experimentosAccion:hover:not(:disabled)`) tiene la clase como sujeto y es correcta.
  - Decidido: `features/mezclador` (duplicado no importado) no se toca en este lote: los archivos divergen, y decidir cuál es canónico exige comparar, no solo borrar. Pendiente en backlog.
- **Sesión 13AA-5 (duplicado mezclador y huérfanas):**
  - Decidido: `frontend/src/mezclador/` es canónica (alias `@mezclador`, consumidor `PanelLateral.tsx`). Eliminado `features/mezclador/` (103 archivos, copia muerta; divergencias solo en la profundidad de import de `crearLogger`).
  - Gate tras eliminar el duplicado: 58 nuevos (`claseHuerfana` en `src/mezclador/styles`) por desfase de rutas del gate (compara por ruta). Resueltos quitando las 76 reglas huérfanas verificadas; gate final 0 nuevos (base 2797, abiertos 1423).
  - Build `C:/tmp/kamples-dist-13aa5b` exit 0; `tsc` del mezclador idéntico a HEAD.
  - Guarda de construcción dinámica: falso positivo por tallo `paso` (coincidía con `paso-${i}` de `graphEdi`); corregido a mano, no en la herramienta todavía (ver backlog).

## Backlog anotado (no olvidar)

Falsos positivos y huecos de herramientas, para el final (actualizar workspace-manager/Sentinel/VarSense):

- Vendorizado: hallazgos en `frontend/src/glory-core/` y `glory-css/` (no se editan).
- `token-duplicado-cross-archivo` en `tokens.css:23-24` (`--fondoElevado2/3: var(--fondoElevado1)`): falso positivo, son alias por `var()`, no valores duplicados; la herramienta no distingue alias de literal repetido.
- `valorHardcoded` marca `var(--x, #literal)` como literal hardcodeado: el fallback no es un valor de diseño. Falso positivo parcial; la herramienta debería ignorar el fallback.
- `valorHardcoded` no cubre el shorthand `border:` con color hex (solo `border-color`): hueco de cobertura, no falso positivo.
- Tablero obsoleto tras cada lote: `board-f3.json` se escaneó antes de `c8540f7`; el forzado debe repetirse por lote.
- `navigationStore.ts:111`: prosa con «todo» sin marcador (posible falso positivo).
- `glory-core/utils/apiCache.ts:14`: singleton mutable, aceptado.
- `PageBuilder.tsx:97`: `any` a revisar; posible falso positivo si es tipo externo.
- `objeto-mutable-exportado`: falso negativo sobre `Readonly<…>` (se resolvió con `Object.freeze`).
- `innerhtml-variable`: el `sentinel-disable` debe usar ese id exacto.
- `key-index-lista`: marca `Array.from().map` con índice en esqueletos estáticos.
- Duplicado `features/mezclador` vs `src/mezclador`: resuelto en 13AA-5 (copia viva: `src/mezclador`). Pendiente: `legacy/islands` vs `features`.
- **Gate por ruta:** `--cierre` compara por ruta y regla; mover o borrar una copia redistribuye hallazgos como nuevos y resueltos (13AA-5: 58 nuevos sin cambio real). Comparar por contenido o hash, o registrar el movimiento.
- **Guarda de construcción dinámica:** el tallo corto (`paso`, `mezclador`) coincide con código no relacionado (`paso-${i}` en `graphEdi`) y omite clases huérfanas reales (13AA-5: 6). Usar tallo completo y límite de token.
- **Selectores mezclados (script 13AA-3/13AA-5):** la regla se quita solo si todos sus selectores están muertos; los selectores huérfanos en listas con clases vivas quedan y producen hallazgos (13AA-5: 4 editados a mano). El script debería quitar solo el selector muerto.
- **Guarda de uso sobre comentarios:** corregido en 13AA-5 (los comentarios ya no cuentan como uso).
- **`Arranque cierre` y ramas retenidas:** `rescate/sentinel-rs-kamples` (`f32fd05`) salía como rama suelta aunque estuviera anotada en roadmap. Decidido: AGENTS §5 solo permite retener ramas por dependencia ajena; esta depende de triaje propio, así que se hizo bundle verificado (`Agente/backup-ramas-2026-10-13/kamples-rescate-sentinel-rs-kamples.bundle`) y se borró local y remoto. Backlog de herramienta: que `--cierre` no cuente como rama suelta una retenida con motivo válido. Hoy no aplica.
- **Rama `rescate/sentinel-rs-kamples` (`f32fd05`):** solo cambia `sentinel.config.json` (incluye `*.rs`). Integrarla antes de triar F4 añade 194 hallazgos nuevos al gate.
- Hallazgo `sinCommit` obsoleto (1) en el tablero.
- Artefacto `(área) :: salud` (1): ruta en `C:\tmp`, no del proyecto.
- `alta` checklist (`registro`): no acepta `noAplica`. Falso positivo en opencode-delegate, que tiene `sin-servicio`.
- `Arranque gate --cierre` exige línea base `--inicio`; proyectos dados de alta después no la tienen.
- **Dos escalas de conteo:** el tablero suma total (1971 en 13AA-1) y el gate cuenta abiertos frente a base (1970 vs 2797 base). Documentar o unificar la métrica para que «problemas» signifique lo mismo en ambos.
- **930 entradas sin desglose por regla** (VarSense y no parseadas): el tablero no las agrupa por regla; el desglose por regla suma ~1.0k. Añadir desglose o clasificar las no parseadas.
- `claseHuerfana` vendorizado (112 en `glory-core`/`glory-css`/`clients`): falso positivo para el proyecto, no editable. Excluir por ruta vendorizada.
- `claseHuerfana` por construcción dinámica (falso positivo): `avatarXs`–`avatar2xl`, `badgeXs`, `badgeNeutro`, `badgeAcento`, `badgeExito`, `variantePrimario`, `varianteGhost`, `variantePeligro`, `tamanoMd`, `adminModeracionDetalle`. El nombre se compone en runtime con prefijo; la herramienta debería reconocer el prefijo o aceptar un patrón declarado.
- `claseHuerfana` sin composición detectada en `frontend/src`: 13AA-3 quitó 268 reglas. Para completar la herramienta: detectar clases por sufijo/prefijo dinámico con tokenizador en vez de búsqueda por substring.
- `valorHardcoded` vendorizado (107): falso positivo para el proyecto.
- `cssInlineReact` (187 editables, 6 vendor): algunos valores son dinámicos (props, cálculos) y no son estilo inline de diseño; distinguir valor literal de expresión.
- Duplicados `features/mezclador` vs `src/mezclador` (alias `@mezclador` en vite.config.ts:16): hallazgos contados en las dos copias. Pendiente: decidir cuál es canónica y borrar la otra por lote (no tocado en 13AA-3). Divergen en ErrorBoundaryMezclador.tsx, useExportarMezcla.ts, useTimeline.ts, motorAudioService.ts, accionesCargaAudio.ts.
- Dependabot de GitHub: 67 vulnerabilidades en la rama por defecto (2 críticas, 35 altas). No gestionadas en el tablero; fuera de alcance de este plan, anotado.
- Proyectos con `PORT=3000` en `.env`/`.env.example` (NAKOMI, PROYECTO TASKS, WANDORIUS, RESTAURANTE): los dueños deciden; el tablero no los señala.

## Definition of Done

- F1 cerrado con verificación §Verificación por lote.
- Hallazgos F2/F3 restantes listados con conteo actualizado en el tablero y enlazados aquí.
- Al cerrar cada fase: entrada en `Agente/completados/tareas-2026-10-13.md`.

### Sesión 13AA-6 (claseHuerfana CSS, pase de verificación)

- Hecho: 65 nombres SIN_USO (grep -w en todo el repo, solo CSS); 68 reglas y 76 selectores quitados en 19 CSS de `frontend/src`. Gate `--cierre`: 0 nuevos, 1351 abiertos. Detalle en [completados 13AA-6](../completados/tareas-2026-10-13.md).
- Backlog de herramienta (nuevo): (a) Sentinel `claseHuerfana` marca 28 clases usadas literalmente en TSX (falsos positivos; causa sin verificar, probablemente nombres pasados por mapa); (b) la guarda de 13AA-3 confunde tallos (`paso`) con construcciones dinámicas; corregir con nombre completo; (c) la guarda de comentarios: el preludio de una regla incluye sus comentarios; tratarlos aparte.
- Decidido: quitar selectores solo si su clase está en la lista SIN_USO y no aparece en ningún archivo no CSS; `:not()`/`:is()` no cuentan como uso. Motivo: un selector con clase ausente del DOM no coincide nunca, así que retirarlo no cambia el render.

### Sesión 13AA-7 (cierre del tablero, decisiones de la usuaria)

- Snapshot forzado tras `4e7dc89` (`C:/tmp/kamples-tablero-13aa7.json`): KAMPLES 1351 abiertos = 988 propios + 363 vendorizados.
- Propios por regla: cssInlineReact 142, inline-style-prohibido 136, window-reference-outside-platform 114, css-hardcoded-value 103, css-especificacion-diseno-local 100, valorHardcoded 91, dom-access-outside-platform 81, cssInlineScript 40, css-adhoc-button-style 33, modal-estructura-no-canonica 32, claseHuerfana 28, css-elemento-html-directo 25, directorio-abarrotado 16, modal-semantica-no-canonica 12, modal-con-titulo 8, limite-lineas 7, componente-artesanal 4, modal-acciones-no-canonico 4, limite-lineas-nivel-2 3, usestate-excesivo 2, componente-sin-hook-glory 2, token-duplicado-cross-archivo 2, mixed-barrel-logic 1, button-clase-especifica 1, token-unused 1.
- Vendorizados por regla: claseHuerfana 112, valorHardcoded 107, css-hardcoded-value 75, console-production 23, window-reference 12, css-adhoc-button-style 8, cssInlineReact 6, dom-access 5, css-elemento-html-directo 5, barras-decorativas 3, css-especificacion-diseno-local 3, todo-prosa-sin-marcador 1, mixed-barrel-logic 1, any-type-explicito 1, singleton-mutable-state 1.
- Decidido (F3 valores): para literales sin token exacto, redondear al token más cercano cuando la diferencia sea pequeña; crear token nuevo cuando esté lejos, de modo que el valor renderizado no cambie. Sin cambio visual salvo redondeos aceptados por la usuaria.
- Decidido (verificación en navegador): base `kamples_verif` en el PostgreSQL local, no `glory_db`. Credenciales pendientes de la usuaria; cuenta admin y contenido de prueba (audio, imagen) generados por el agente.
- Decidido (vendorizados, 363): tarea upstream aparte en su repo de origen; no se editan copias vendorizadas ni se excluyen en silencio de `sentinel.config.json`.

### Sesión 13AA-8 (copia muerta `frontend/src/features/`)

- Comprobado antes de borrar: ningún import vivo (grep de `features/` solo en docs y `tsconfig.json`). 37 de 39 `.tsx/.ts` idénticos a su equivalente en `legacy/islands/`; `player/ReproductorIsland.tsx` y `samples/SampleDetalleIsland.tsx` son versiones antiguas (la viva tiene los arreglos).
- Eliminado con `git rm` (40 archivos, incluido `features/README.md`). Quitada la exclusión `src/features/**/*` de `frontend/tsconfig.json`. Recuperable desde git.
- Evidencia: `vite build` OK; `tsc --noEmit` con 3 errores preexistentes en `src/legacy/hooks/useAuth.ts` (`require` sin `@types/node`), ajenos al cambio; gate `--cierre`: 0 nuevos, tablero 1158 → 1110.
- Pendiente: los errores `require` de `useAuth.ts` (tarea aparte).

## Sesión 13AA-9 (estilos inline estáticos a CSS)

- Alcance: 49 `style={{...}}` estáticos en 26 TSX (sitios vivos en `legacy/` y `mezclador/`), movidos a `legacy/styles/componentes/<stem>.css` con clase `<stem>-inlN`. Valores iguales salvo redondeos a token.
- Excluidos (dinámicos, falsos positivos para la herramienta): `VisualizadorOnda.tsx` (alto variable), `ComunidadIsland`, `LibreriaIsland` y otros `style` con expresiones.
- Redondeos (revisar visualmente en localhost con `kamples_verif`): `0.85rem` → `--fuenteMd` (13px, en `modalSuspenderAdmin.css` y `modalCancionAleatoria.css`); `0.8rem` → `--fuenteSm` (12px); `#1a1a1a`, `#9ca3af` → tokens nuevos `--colorRgba-26-26-26-1` y `--colorRgba-156-163-175-1` en `styles/tokens.css` (bloque 13AA-7); `#ef4444` → `--colorRojoError`; `8px` de radio → `--espacioSm` (mismo valor, sin radio propio de 8px).
- Gate: `--inicio` base 2797; `--cierre` tras los lotes: 19 nuevos (CSS nuevo sin tokens), corregidos; `--cierre` final: 0 nuevos, 1012 abiertos.
- Build: `vite build` OK.
- Pendiente: verificación visual en navegador (bloqueada hasta credenciales de `kamples_verif`).

## Sesión 13AD (console-production y backlog de herramienta)

- Decidido: consola del framework glory-core pasa por `src/glory-core/utils/logger.ts`. `error` siempre; `warn/info/debug` solo en `import.meta.env.DEV`. Cambio de comportamiento: en producción ya no aparecen los avisos diagnósticos `[Glory]`. Sin cambio visual.
- 8 archivos migrados (23 llamadas): `core/ErrorBoundary.tsx`, `core/hydration.tsx`, `core/IslandRegistry.ts`, `hooks/useGloryContent.ts`, `hooks/useGloryContext.ts`, `pageBuilder/BlockRegistry.ts`, `pageBuilder/BlockRenderer.tsx`, `pageBuilder/hooks/usePageBuilder.ts`.
- Excepción: las 4 llamadas de `logger.ts` llevan `sentinel-disable-next-line console-production`, porque es el único punto de salida a consola. Cuentan como hallazgo aceptado en el tablero.
- Build OK; tsc sin errores nuevos (solo los 3 preexistentes de `useAuth.ts`).

## Sesión 13AE (genuinos sin cambio visual: token, any, singleton)

- Commit `a59e462` en `main` (pusheado): `tokens.css` quita `--fondoElevado2/3` duplicados de `legacy/styles/variables.css`; `PageBuilder.tsx` cast sin `any` ni `eslint-disable`; `apiCache.ts` estado privado (`leerCacheApi`/`guardarCacheApi`); `useWordPressApi.ts` usa esas funciones.
- Type-check: solo los 3 errores preexistentes de `legacy/hooks/useAuth.ts` (TS2591 `require`).
- Gate `--cierre`: 0 nuevos; base 2797, ahora 980 abiertos (1817 resueltos).
- Sin cambio visual: ninguno de estos cambios altera el render.

### Backlog de la herramienta (anotado, para el final)

Falsos positivos o ruido a corregir en `workspace-manager` cuando se actualice la herramienta:
- `claseHuerfana` (140 en KAMPLES): clases usadas por composición dinámica (`className={\`x-${v}\`}`) o por nombres en objetos; el detector no las ve.
- Estilos inline dinámicos (`style={{ alto: variable }}`) contados como `inline-style-prohibido`/`cssInlineReact`, aunque no tienen equivalente estático (p. ej. `VisualizadorOnda`, `ComunidadIsland`, `LibreriaIsland`).
- Guard 13AA-3 confunde el stem `paso` con otro componente.
- Guard de comentarios: marca `sentinel-disable-*` y comentarios explicativos como problemas.
- `--cierre` cuenta ramas retenidas como sueltas aunque estén integradas.
- Nombres mecánicos de tokens (`--colorRgba-26-26-26-1`) que la herramienta no distingue de tokens semánticos.
- Fallbacks redundantes `var(--x, var(--x))` no detectados como problema en algunos casos.
- Excepción sentinel documentada (`logger.ts`) sin forma de marcarla como aceptada.

### Pendiente real (propios, sin cambiar en esta sesión)

- `window-reference-outside-platform` (116) y `dom-access-outside-platform` (86): refactor de comportamiento con pruebas funcionales antes y después.
- `css-especificacion-diseno-local` (103), `css-adhoc-button-style` (41), `css-elemento-html-directo` (30), `modal-*` (~50).
- `directorio-abarrotado` (16), `limite-lineas` (7) y `limite-lineas-nivel-2` (3).
- `componente-artesanal` (4), `usestate-excesivo` (2), `mixed-barrel-logic` (2), `token-duplicado-cross-archivo` (2), `singleton-mutable-state` (1), `any-type-explicito` (1), `button-clase-especifica` (1), `componente-sin-hook-glory` (1).
- Verificación visual en `kamples_verif` (estado 2026-10-13, desbloqueada): BD creada y migrada (41 tablas); backend en `127.0.0.1:3001` (`KAMPLES_WORKERS_ENABLED=false`) y frontend en `5183`. Cuentas de prueba en localhost: `admin_verif` (rol admin, login OK, JWT con `rol: admin`) y `usuario_verif` (registro OK vía `/api/auth/register`). Credenciales en `C:/tmp/kamples-verif-admin.txt`, fuera del repo.
- Verificación de contenido: upload de WAV de prueba (`POST /api/samples/upload` → 201). El sample no aparece en el feed ni en el panel admin («No hay canciones registradas»): los workers de audio están desactivados, así que no se procesa. Imagen de portada aplicada (miniatura visible en el modal de subida).
- Pendiente de verificación visual de 13AA-9 (`appIslands.css`, `modalCancionAleatoria.css`, `modalSuspenderAdmin.css`): `ModalCancionAleatoria` y `VisualizadorOnda` requieren canciones procesadas (habilitar workers); `ModalSuspenderAdmin` requiere la lista de usuarios del panel admin, que aparece vacía pese a que `GET /api/admin/usuarios` devuelve 2 usuarios (hallazgo a investigar: render de la tabla en `/admin/panel/`).
- Consola del navegador: errores 401/404/405 y `fetchPriority` en `LandingPublica.tsx` (prop `fetchPriority` no reconocida; preexistente, no tocado).
- Decidido: la contraseña genérica de Postgres local (`root`, localhost) la autorizó la usuaria para `kamples_verif`; se usa solo para esa BD en localhost, nunca en chat ni en el repo. Pendiente: reiniciar el backend 3001 (proceso propio) con `KAMPLES_WORKERS_ENABLED=true` para procesar el WAV de prueba; no se ha hecho.
- Nota: se paró una consulta psql en segundo plano (proceso propio) que dejó el resultado vacío; no se reinició.

## Sesión 13AF (frontera window/document: `window-reference-outside-platform` y `dom-access-outside-platform`)

Decidido: arquitectura por adaptadores, no comentarios ni `sentinel-disable`. Toda referencia a `window`/`document` vive bajo `frontend/src/glory-core/platform/` (regla de `sentinel.config.json` `portableBoundaries`).

- `platform/ventana.ts`: `ubicacionActual`, `rutaActual`, `busquedaActual`, `origenActual`, `protocoloActual`, `irAUrl`, `volverAtras`, `agregarEntradaHistorial`, `reemplazarEntradaHistorial`, `anchoVentana`, `altoVentana`, `desplazarVentanaA`, `escucharVentana`, `dejarDeEscucharVentana`.
- `platform/documento.ts`: `cuerpoDocumento`, `cabezaDocumento`, `crearElemento`, `consultarElemento`, `consultarElementos`, `elementoPorId`.
- Sobrecargas añadidas: `escucharVentana`/`dejarDeEscucharVentana` aceptan `string` para eventos personalizados (`kamples:...`, no están en `WindowEventMap`). `desplazarVentanaA` acepta `ScrollToOptions` además de `(x, y)`.
- `useBackHandler.ts`: `rutaActual` local (string) se mantiene; el alias `rutaVentana` solo se usa para leer la ruta del navegador (`?? rutaVentana()`), así no hay ambigüedad entre estado y ventana.
- Migración: ~90 archivos en `legacy/**`, `mezclador/**`, `glory-core/**`, `hooks/**`, `components/**`, `router.tsx`, `main.tsx`. Verificado por grep: ninguna referencia fuera de `platform/` (solo comentarios).
- Gate `--cierre` (2026-10-13): 0 nuevos, 1998 resueltos, 799 abiertos (base 2797).
- Type-check: solo los 3 errores preexistentes de `legacy/hooks/useAuth.ts` (TS2591).
- Build: `npm run build` falla antes de Vite por `scripts/gen-colors-list.mjs` (ENOENT en `public/legacy-assets/colors`, gitignored, preexistente). Decidido: no tocar assets ignorados; `npx vite build` compila sin errores (solo advertencias `INEFFECTIVE_DYNAMIC_IMPORT` en `clients/desktop/src/services/*`, preexistentes). Tarea aparte: hacer que el script del build tolere el asset ausente o documentar el paso manual.
- Cliente de escritorio (16 hits en `clients/desktop/src/*`): no cubierto por esta migración; pendiente de confirmar si `@desktop` alcanza `@/platform` antes de editar.
- Pendiente de verificación visual de esta frontera: sin cambio visual esperado. Falta recorrer en `kamples_verif` los flujos que tocan ruta, scroll, menús contextuales y reproductor.

### Pendiente de este plan (siguiente bloque)

- `css-especificacion-diseno-local` (103) y `css-adhoc-button-style` (41): cambian render; requieren verificación visual antes y después.
- `directorio-abarrotado` (16): reorganizar carpetas con reescritura de imports (riesgo de disparar la regla en la carpeta nueva; precedente `Agente/completados/tareas-2026-10-07.md:350`).
- Estructurales: `limite-lineas` (7) y `limite-lineas-nivel-2` (3).

## Sesión 13AF-cont: cliente desktop window/document (commit `887f061`)

- Migrados 8 archivos de `clients/desktop/src` a `@/platform/ventana` y `@/platform/documento` (17 sustituciones, sin cambio de comportamiento). Type-check desktop: 0 errores.
- Gate `--cierre`: 0 nuevos, 2014 resueltos (base 2797, ahora 783).
- Tablero KAMPLES tras el análisis: `window-reference-outside-platform` 5 y `dom-access-outside-platform` 0. Los 5 restantes son comentarios (falsos positivos ya anotados: `LandingPublica.tsx:45`, `useAuth.ts:183,196`, `usePlanesIsland.tsx:170`, `photonUrl.ts:8`).
- Pendiente, en este orden: `directorio-abarrotado` (16 carpetas, reorganizar por dominio con reescritura de imports; no usar `directoryExceptions` para ocultarlo), `limite-lineas` (7) y `limite-lineas-nivel-2` (3), `css-especificacion-diseno-local` (103) y `css-adhoc-button-style` (41) con verificación visual en `kamples_verif`.
- Pendiente de anotar en el backlog de falsos positivos: `claseHuerfana` (140), `cssInlineReact` (81), `cssInlineScript` (40), `inline-style-prohibido` (69) cuando el origen sea estilo dinámico.
