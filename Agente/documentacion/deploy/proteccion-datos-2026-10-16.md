# Protección de datos: base de datos y archivos físicos (2026-10-16)

Viñeta de roadmap: Fase 19, «un error pasado hizo que se borrara la base de
datos… se debe tomar todos los medios necesarios para evitar la pérdida de
datos, tanto de la base de datos como de los archivos físicos que deben
perdurar». Esta es la fuente canónica de ese bloque; el resto de documentos
enlazan aquí.

## Qué pasó de verdad

El incidente no fue «no había copia». Fue una **restauración que dejó la base
vacía**: el dump existía, tenía un nombre plausible y estaba vacío o truncado.
El diagnóstico de `plan-kamples-diagnosis-2026-06-16.md` ya lo dejó escrito
(«pérdida de datos anterior al backup», restauraciones previas que crearon
bases vacías).

Al revisarlo aparecieron dos defectos concretos en los scripts del repo:

1. `scripts/backup-postgres.sh` volcaba con
   `docker compose exec -T postgres pg_dump … > "$OUT"`. Si el `exec` falla, el
   redirect **ya ha creado `$OUT`**: queda un archivo de 0 bytes con un nombre
   que parece un backup. Nada lo distinguía de uno bueno.
2. La instrucción de restauración del runbook de deploy era
   `pg_restore --clean --if-exists` **sin comprobar el dump antes**. Sobre un
   dump vacío, `--clean` borra la base actual y después no repone nada.

Y un tercero, de fondo: nada obligaba a que el volcado se verificara. El
sidecar tampoco verificaba.

## La regla

> Un backup que no se verifica no es un backup, y una restauración que no
> verifica antes de tocar la base es la forma más rápida de perderla.

Todo lo que decide si un archivo `.dump` sirve vive en un solo sitio:
`scripts/lib/dump-postgres.sh`. Se comparte entre el camino manual y el
automático a propósito, para que no puedan divergir.

Un dump se acepta si:

- pesa más de `DUMP_MIN_BYTES` (512 B, solo red contra el archivo de 0 bytes);
- `pg_restore --list` lo lee sin error (valida la cabecera y el índice);
- su índice tiene **al menos una entrada `TABLE DATA`**.

El tercero es el criterio de verdad, no el peso. Medido en la 89 con `pg_dump`
real: una base recién creada y sin tablas pesa **913 bytes** y una con una tabla
de 500 filas, **3416**. Un umbral «generoso» de 4096 bytes —el que se puso
primero— rechazaba bases legítimas pequeñas. La firma de una base vacía no es
el tamaño: es no tener ninguna entrada `TABLE DATA`.

## Los scripts

| Script | Qué hace |
|---|---|
| `scripts/backup-postgres.sh [etiqueta]` | Volcado manual. Escribe a `.partial`, verifica y solo entonces renombra. Un fallo no deja nada que parezca un backup. |
| `scripts/backup-postgres-loop.sh` | Sidecar cada 6 h. Verifica antes de aceptar, avisa tras 3 fallos seguidos y si el último bueno tiene más de dos intervalos. La rotación **conserva siempre el más reciente**, aunque supere la retención. |
| `scripts/restore-postgres.sh <dump> [--aplicar] [--forzar]` | La única vía sancionada de restaurar. Sin `--aplicar` es simulación. |
| `scripts/backup-uploads.sh [etiqueta]` | Copia manual de `/data/kamples/uploads`. Comprueba que el tar trae el mismo número de archivos que el origen. |
| `scripts/backup-uploads-loop.sh` | Sidecar cada 24 h de los **archivos físicos**. Mismas garantías que el loop de la base: verifica antes de aceptar, avisa tras 3 fallos y si el último bueno tiene más de dos intervalos, y rota conservando siempre el más reciente. Monta los uploads en **solo lectura**. |
| `scripts/lib/tar-uploads.sh` | Reglas de verificación de las copias de uploads, compartidas por el script manual y el sidecar para que los dos caminos no puedan divergir. |
| `scripts/migrar.sh <archivo.up.sql>…` | Guardia → copia verificada → aplicar. |
| `scripts/guardia-migraciones.sh [archivos…]` | Se niega a dar por bueno SQL destructivo no declarado. |

### Garantías de `restore-postgres.sh`

1. Verifica el dump **antes** de tocar nada. Un dump vacío o truncado se
   rechaza aquí.
2. Saca copia de la base **actual**, la verifica, y si no sale bien **aborta
   sin tocar la base**.
3. Si la base destino tiene tablas y no se pasa `--forzar`, se niega.
4. Restaura con `--single-transaction`: si falla a medias, PostgreSQL la
   deshace entera. Nunca queda una base a medio poblar, que es indistinguible
   de una base perdida.
5. Al terminar comprueba que quedaron tablas y, si no, dice dónde está la copia
   previa.

### Garantías de `migrar.sh`

1. Guardia de SQL destructivo (`migrations/.destructivas-permitidas`).
2. Copia de seguridad verificada. **No hay flag para saltársela.**
3. `psql -v ON_ERROR_STOP=1 --single-transaction`. El `psql` a mano que
   documentaba el roadmap no paraba ante un error: seguía aplicando el resto
   del archivo sobre un esquema a medias.

## El ejecutor dentro del contenedor

Los scripts no hablan con PostgreSQL directamente: lo hacen a través de
`PG_EXEC`. Por defecto `docker compose exec -T postgres`, que sirve donde hay
acceso al host del stack. **En producción AGENTS §10 prohíbe el SSH directo y el
Docker remoto**, así que allí es obligatorio el de coolify-manager-rs:

```bash
PG_EXEC='cm exec --name kamples --target postgres --' ./scripts/restore-postgres.sh <dump> --aplicar
```

Es la única pieza que cambia entre entornos, y por eso es una variable.

## Pruebas

`npm run bd:test` — 25 casos, shell puro, sin base de datos ni servidor:

- `dump_verificar`: 8 casos (acepta un dump bueno; rechaza inexistente, 0 bytes,
  truncado, índice vacío y base sin esquema; umbral configurable; `PG_EXEC`
  multilínea).
- `migracion_es_destructiva`: 17 casos, con los negativos a propósito
  (`DROP INDEX`, `DROP CONSTRAINT`, `ADD COLUMN`, `DELETE … WHERE`, un
  comentario que menciona `DROP`) para que la guardia no acabe ignorándose por
  ruido, y el testigo de que detecta la migración destructiva real del repo.

Además, contra `pg_dump`/`pg_restore` **reales** en la 89 (5 casos, base
desechable `claude_prueba_150`, ya borrada): la base vacía de 913 B se rechaza,
la de 3416 B con datos se acepta, el mismo dump truncado a 100 B se rechaza, el
mismo dump con la cabecera pisada se rechaza, y una base con esquema y 0 filas
se acepta (sin falso positivo).

`npm run bd:guardia` — sobre las 24 migraciones reales: 1 destructiva, ya
declarada; **ninguna de las otras 23 da falso positivo**.

`npm run archivos:test` — 30 casos sobre `tar`/`find` reales, sin base de datos:

- `uploads_verificar`: acepta un tar completo y un origen vacío legítimo;
  rechaza inexistente, 0 bytes, **truncado a la mitad** (el caso que importa: un
  gzip cortado no se puede listar), un tar al que le falta un archivo, bytes
  cualesquiera y el umbral de tamaño subido.
- `uploads_nombre_valido`: acepta `uploads-*.tar.gz` y rechaza un dump, un
  `.partial` y un tar ajeno, para que un `find -delete` no se lleve por delante
  algo que no es nuestro.
- `backup-uploads.sh`: copia en verde sin dejar `.partial`, fallo loud con origen
  inexistente sin tocar las copias anteriores, fallo sin restos si el destino no
  es escribible, y rotación que borra las vencidas **conservando la más
  reciente**.
- `backup-uploads-loop.sh`: ejecutado de verdad con `/bin/sh` y directorios
  temporales — deja copias verificadas, escribe la marca de última copia buena,
  **no da aviso de ranciedad estando sano** (la regresión del umbral) y muere si
  no existe el origen.

**Lo que estas pruebas NO cubren.** La 89 no tiene Docker, así que el sidecar
**no se ha ejecutado dentro del contenedor**: se ejecutó el mismo script con
`/bin/sh` sobre directorios temporales. Queda sin probar busybox (frente a GNU
coreutils), el montaje `:ro` y que la imagen `postgres:16-alpine` traiga los
binarios que el loop usa. Se argumenta por construcción —POSIX sh y juego de
herramientas busybox—, no por ejecución.

`npm run compose:test` — guarda contra un fallo que apareció al hacer esto:
`.gitignore` tiene `/scripts/*` con lista blanca, así que un script nuevo queda
**fuera del repo en silencio** salvo que se le añada su `!`. A
`backup-uploads-loop.sh` le pasó: el compose lo montaba y el repo no lo llevaba,
de modo que en un clon limpio el sidecar no arrancaría y —como el resto del
stack sigue en pie— **parecería que hay copias cuando no hay ninguna**. La guarda
saca del compose real las rutas `./scripts/…`, exige que existan y, donde hay
git, que estén versionadas. Incluye testigo: si el barrido no encuentra ninguna
ruta (el compose cambió de forma), falla en vez de pasar por vacío.

`npm run scripts:test` — las tres suites seguidas (`bd:test`, `archivos:test`,
`compose:test`).

## Lo que sigue faltando

- **Copia off-site.** Hoy todo vive en `/data/kamples` del mismo servidor: si se
  pierde el disco, se pierde todo. Pendiente en
  `plan-deploy-coolify-2026-06-30.md:350` y
  `Agente/documentacion/deploy/secrets-2026-04-20.md:153`. **Decisión de la
  usuaria** (destino, credenciales).
- **Ejercicio de restauración real.** Los scripts están probados por partes y
  contra `pg_dump` real, pero nunca se ha hecho un restore completo contra el
  stack de producción. Requiere autorización por operación (producción).
- **Migraciones sin `.down.sql`.** De 24 migraciones, solo una tiene su
  reverso (`20260703000000_fix_colecciones_schema.down.sql`). No hay plan de
  vuelta para el resto.

## Copias de los archivos físicos

Cerrado el 2026-10-16 (16A-7). Hasta entonces **la base se copiaba cada 6 h pero
los uploads solo tenían script manual**, así que en la práctica los samples,
avatares y portadas podían llevar meses sin copia: el sidecar cubría una sola de
las dos cosas que hay que preservar.

- El tar se empaqueta con `-C` al padre del directorio de uploads, así que
  **dentro no hay rutas absolutas** y se extrae donde haga falta:
  `tar xzf uploads-….tar.gz -C <padre-de-uploads>`.
- El sidecar monta `/data/kamples/uploads` en **solo lectura**: empaqueta, nunca
  escribe ni borra en el directorio de uploads.
- Se reutiliza la imagen `postgres:16-alpine` a propósito, para no añadir otra
  imagen al stack: trae busybox con `tar`/`find`/`stat`, que es todo lo que
  necesita el loop.
- El umbral de ranciedad se redondea **hacia arriba** a minutos enteros. Con
  `INTERVAL/60*2` y un intervalo corto daba 0, y `find -mmin +0` es cierto para
  casi todo: el aviso quedaba encendido siempre y no avisaba de nada. Mismo
  defecto corregido en `backup-postgres-loop.sh`, que lo tenía igual.
