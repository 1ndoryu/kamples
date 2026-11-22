# Instalación de escritorio de Kamples (Windows)

La app vive en una carpeta fija del portátil, separada de los datos. Actualizar es sustituir el
programa; los datos no se tocan nunca.

| Pieza | Ruta |
|---|---|
| Programa | `C:\Users\Owner\Apps\KAMPLES` |
| Datos | `%LOCALAPPDATA%\KamplesDatos` |
| Acceso directo | `Desktop\Kamples.lnk` (apunta al `.exe`, icono del propio `.exe`) |

## Qué hay en la carpeta del programa

```
KAMPLES\
├─ Kamples.exe            lanzador Tauri (ventanas, tray, arranque)
├─ glory-backend.exe      backend Axum; sirve el front, /api, /uploads y el pipeline
├─ front\                 front React compilado
├─ migraciones\           24 migraciones SQL
├─ bin\                   ffmpeg y ffprobe
├─ pgsql\                 PostgreSQL 18 portable con pgvector
└─ LEEME.txt              instrucciones para la usuaria
```

## Qué hay en la carpeta de datos

```
%LOCALAPPDATA%\KamplesDatos\
├─ pgdata\        cluster PostgreSQL (base de samples, usuarios, colecciones)
├─ uploads\       audio subido y sus derivados (MP3 320, waveform)
├─ backups\       pg_dump -Fc diario, se conservan 14
├─ logs\          backend.log, postgres.log
└─ secretos.txt   JWT y WS de 32 bytes; si se borra, se cierran todas las sesiones
```

El nombre `KamplesDatos` es deliberado: el instalador NSIS de Windows usa `Kamples` para el
programa y su desinstalador borra esa carpeta entera.

## Actualizar la app

1. Compilar en la 89 y traer el binario nuevo (nunca en el portátil):

   ```powershell
   node workspace-manager/scripts/remoto/rcargo.mjs --proyecto=KAMPLES xwin build --release --target x86_64-pc-windows-msvc --manifest-path clients/desktop/src-tauri/Cargo.toml --bin kamples-desktop --prepath=/usr/lib/llvm-18/bin
   ```

2. Parar la app (bandeja → Salir) y comprobar que el 4317 quedó libre.
3. Sustituir solo los archivos de programa que cambiaron: `Kamples.exe`, `glory-backend.exe`,
   `front\`, `bin\`, `pgsql\`, `migraciones\`. Las migraciones nuevas se aplican solas en el
   siguiente arranque (tabla de control `_kamples_migraciones`).
4. Abrir de nuevo. `/api/health` en `http://127.0.0.1:4317/api/health` debe responder
   `{"status":"ok","version":"…"}`.

Nunca borrar ni mover `KamplesDatos` para actualizar.

## Comprobar que arrancó

```powershell
(Invoke-WebRequest 'http://127.0.0.1:4317/api/health' -UseBasicParsing).Content
```

Puerto fijo del backend: 4317 (las capabilities de Tauri lo declaran como origen remoto exacto, así
que no se puede cambiar sin tocar `capabilities/*.json`). PostgreSQL toma el primer puerto libre
desde 5433.

## Instalación anterior

El instalador NSIS de marzo de 2026 (`C:\Users\Owner\AppData\Local\Kamples`,
`kamples-desktop.exe` + `uninstall.exe`) se desinstaló el 2026-11-21 con su propio
`uninstall.exe` (código 0): carpeta, clave HKCU de desinstalación y acceso del menú Inicio
borrados, sin restos en AppData. `KamplesDatos` no se tocó, porque el desinstalador borra
`Kamples` y no `KamplesDatos`.
