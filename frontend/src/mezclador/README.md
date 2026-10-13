# Mezclador (DAW)

Origen: `glorytemplate/Mezclador/` (legado WordPress).

[174A-109b] Movido 1:1 a `frontend/src/features/mezclador/` (era `clients/Mezclador/`); duplicado eliminado en 13AA-5, la copia viva es `frontend/src/mezclador/` (alias `@mezclador`). El `tsconfig.json` interno se eliminó porque ya está cubierto por el del frontend, que excluye `src/features/**/*` del type-check global hasta que cada feature complete su adaptación.

## Estado actual

- ✅ Código importado y ubicado físicamente en su destino final.
- ✅ Compila en validación dedicada con `frontend/tsconfig.mezclador.json`.
- ✅ Las dependencias legacy `@app/*` ahora resuelven contra una capa mínima de compatibilidad en `frontend/src/app/`.
- ✅ El browser del DAW ya no depende de `wp-json/...`: `@app/services/apiExplorador` adapta `GET /api/samples` vía Orval para recuperar carpetas y samples públicos.
- ❌ NO está conectado a ninguna ruta de la SPA.

## Pendientes (sub-tareas reagendadas)

1. **174A-109b-fase4 — montar ruta SPA:**
   - Añadir `<Route path="/mezclador" element={<MezcladorPanel />} />` al router.
   - Lazy-load la feature porque pesa ≥100 archivos + dependencias de audio (`soundtouchjs`, `lucide-react`).
2. **174A-109b-fase5 — quitar `exclude: src/features/**/*` del `tsconfig.json` cuando todas las features compilen.**

## Notas de adaptación

- La compatibilidad `@app/*` es intencionalmente mínima: solo cubre stores, tipos y componentes que el Mezclador consume hoy.
- `@app/services/apiExplorador` agrupa el catálogo público por `tipo` para reconstruir carpetas navegables sin reintroducir el backend WordPress. Si el backend Rust incorpora un explorador más rico, esa mejora debe hacerse dentro de este adaptador, no volviendo a `wp-json/...`.

## Estructura

```
mezclador/
  components/         (ChannelRack, Mixer, PianoRoll, BloqueSample, …)
  hooks/              (useMezclador, useBrowserDaw, …)
  services/           (audio engine + persistencia)
  stores/             (Zustand: mezcladorStore)
  styles/             (CSS in espanol camelCase)
  types/
  utils/
  featureFlags.ts
```
