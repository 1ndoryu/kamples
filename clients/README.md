# Clients

Clientes externos del proyecto, importados 1:1 desde el legado WordPress
(`glorytemplate/{kamples-scraper,Mezclador,mobile,desktop}`) en
**174A-108..111**.

[174A-109b] `Mezclador` vive en `frontend/src/mezclador/` (antes `features/mezclador/`, duplicado eliminado en 13AA-5)
porque es una feature in-process del front, no un cliente externo.

## Estado

| Cliente          | Stack                          | Estado                    |
|------------------|--------------------------------|---------------------------|
| `kamples-scraper`| Python + Scrapy                 | [174A-108b] Migrado a `/api/admin/scraper/{publicar-auto,reporte-lote}` |
| `mobile`         | Capacitor (Android) + WebView   | Importado, apunta a la SPA en producción |
| `desktop`        | Tauri 2 + React                 | Importado, usa `wpApiSettings` simulado |

## Plan de adaptación

Ver `Agente/planes/plan-clients-adapters-2026-04-20.md` para el detalle de
qué hay que cambiar en cada cliente para pasar de WordPress al backend Axum.

## Por qué viven aquí y no en repos separados

- Versionado conjunto con el backend (un commit puede tocar API + cliente).
- Permite que `cargo` / `npm` corran en CI sobre todo el contrato.
- Cuando un cliente alcance estabilidad, se puede extraer a su propio repo
  con `git filter-repo` sin perder historial.

## Reglas

- **No editar manualmente** archivos generados (capacitor `android/`, tauri
  `target/`, scrapy `.scrapy/`).
- Cada cliente tiene su propio `package.json` / `requirements.txt` —
  instalar dependencias en su carpeta.
- Las URLs base se configuran por **variables de entorno**, nunca
  hardcodeadas.
