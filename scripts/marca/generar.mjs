/*
 * Genera las constantes de marca desde `marca.json`.
 *
 * [por que] El nombre de la app aparece en Rust, TypeScript, HTML y en la
 * configuración de Tauri. Escrito a mano en cada sitio, cambiarlo es una caza
 * de literales y uno se queda atrás. Aquí hay una sola fuente y este script
 * emite los ficheros derivados; están marcados como generados y no se editan.
 *
 * Uso:  node scripts/marca/generar.mjs
 *       node scripts/marca/generar.mjs --revisar   (falla si algo está desfasado)
 */

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const revisar = process.argv.includes('--revisar');

const marca = JSON.parse(readFileSync(join(RAIZ, 'marca.json'), 'utf8'));
const { nombre, nombreCorto, identidad } = marca;

/* Un nombre con comillas o saltos rompería el código generado; se corta aquí,
 * en el generador, y no en cada consumidor. */
for (const [clave, valor] of Object.entries({ nombre, nombreCorto })) {
  if (typeof valor !== 'string' || !/^[\w \-.&']{1,40}$/u.test(valor)) {
    console.error(`marca.json: "${clave}" no es un nombre válido: ${JSON.stringify(valor)}`);
    process.exit(1);
  }
}
for (const clave of ['slug', 'identifier', 'esquemaEnlace', 'carpetaDatos', 'dominio']) {
  if (typeof identidad?.[clave] !== 'string' || !identidad[clave]) {
    console.error(`marca.json: falta identidad.${clave}`);
    process.exit(1);
  }
}

const AVISO = 'GENERADO por scripts/marca/generar.mjs desde marca.json — no editar a mano.';

const rust = `#![allow(dead_code)] // constantes de marca: cada crate usa unas pocas
/* ${AVISO}
 *
 * [por que] El nombre visible y lo que identifica a la app en disco son cosas
 * distintas: el nombre puede cambiar cuando se decida la marca definitiva, pero
 * \`IDENTIDAD_*\` no, porque la carpeta de datos, la base local y el
 * \`identifier\` de Tauri ya existen en los equipos con datos dentro.
 */

/// Nombre visible de la app: títulos, bandeja, correos.
pub const NOMBRE: &str = "${nombre}";
/// Nombre corto, para donde el largo no cabe.
pub const NOMBRE_CORTO: &str = "${nombreCorto}";

/// Identidad fija: cambiarla exige migrar los datos existentes.
pub const IDENTIDAD_SLUG: &str = "${identidad.slug}";
pub const IDENTIDAD_CARPETA_DATOS: &str = "${identidad.carpetaDatos}";
pub const IDENTIDAD_DOMINIO: &str = "${identidad.dominio}";
/// Sitio público, para enlaces en correos y valores por defecto.
pub const SITIO_URL: &str = "https://${identidad.dominio}";
`;

const ts = `/* ${AVISO} */

/** Nombre visible de la app: títulos, cabeceras, textos. */
export const NOMBRE = '${nombre}';
/** Nombre corto, para donde el largo no cabe. */
export const NOMBRE_CORTO = '${nombreCorto}';

/** Identidad fija: cambiarla exige migrar los datos existentes. */
export const IDENTIDAD_SLUG = '${identidad.slug}';
export const IDENTIDAD_DOMINIO = '${identidad.dominio}';
/** Esquema de los enlaces profundos; parte de la identidad. */
export const IDENTIDAD_ESQUEMA_ENLACE = '${identidad.esquemaEnlace}';
/** Cabecera propia de la API: contrato con el backend, viaja con la identidad. */
export const CABECERA_AUTH = 'X-${identidad.slug.charAt(0).toUpperCase()}${identidad.slug.slice(1)}-Auth';
`;

/** Tauri: productName e identifier salen de la marca, sin duplicarlos. */
function parchearTauriConf() {
  const ruta = join(RAIZ, 'clients', 'desktop', 'src-tauri', 'tauri.conf.json');
  const texto = readFileSync(ruta, 'utf8');
  const conf = JSON.parse(texto);
  const cambios = [];
  if (conf.productName !== nombre) cambios.push(['productName', conf.productName, nombre]);
  if (conf.identifier !== identidad.identifier) {
    cambios.push(['identifier', conf.identifier, identidad.identifier]);
  }
  /* El esquema del deep-link móvil (`kamples://auth`) es parte de la identidad:
   * un `intent-filter` de Android ya publicado no se puede renombrar gratis. */
  const movil = conf.plugins?.['deep-link']?.mobile?.[0];
  const esquemaMovil = movil?.scheme?.[0];
  if (esquemaMovil !== undefined && esquemaMovil !== identidad.esquemaEnlace) {
    cambios.push(['plugins.deep-link.mobile[0].scheme[0]', esquemaMovil, identidad.esquemaEnlace]);
  }
  if (cambios.length === 0) return [];
  if (revisar) return cambios;
  conf.productName = nombre;
  conf.identifier = identidad.identifier;
  if (esquemaMovil !== undefined) movil.scheme[0] = identidad.esquemaEnlace;
  writeFileSync(ruta, `${JSON.stringify(conf, null, 2)}\n`, 'utf8');
  return cambios;
}

/** Android: el título de la ventana en `tauri.android.conf.json`. */
function parchearAndroidConf() {
  const ruta = join(RAIZ, 'clients', 'desktop', 'src-tauri', 'tauri.android.conf.json');
  if (!existsSync(ruta)) return [];
  const conf = JSON.parse(readFileSync(ruta, 'utf8'));
  const ventana = conf.app?.windows?.find((v) => v.label === 'main');
  if (!ventana || ventana.title === nombre) return [];
  const cambios = [['tauri.android.conf.json app.windows[main].title', ventana.title, nombre]];
  if (!revisar) {
    ventana.title = nombre;
    writeFileSync(ruta, `${JSON.stringify(conf, null, 2)}
`, 'utf8');
  }
  return cambios;
}

const salidas = [
  ['src/marca.rs', rust],
  ['clients/desktop/src-tauri/src/marca.rs', rust],
  ['frontend/src/marca.ts', ts],
];

let desfasados = 0;
for (const [relativa, contenido] of salidas) {
  const ruta = join(RAIZ, relativa);
  const actual = existsSync(ruta) ? readFileSync(ruta, 'utf8') : null;
  if (actual === contenido) continue;
  if (revisar) {
    desfasados += 1;
    console.error(`desfasado: ${relativa}`);
    continue;
  }
  writeFileSync(ruta, contenido, 'utf8');
  console.log(`escrito: ${relativa}`);
}

for (const [campo, antes, despues] of [...parchearTauriConf(), ...parchearAndroidConf()]) {
  if (revisar) {
    desfasados += 1;
    console.error(`desfasado: tauri.conf.json ${campo}: ${antes} → ${despues}`);
  } else {
    console.log(`tauri.conf.json ${campo}: ${antes} → ${despues}`);
  }
}

if (revisar && desfasados > 0) {
  console.error(`\n${desfasados} fichero(s) desfasados respecto a marca.json.`);
  process.exit(1);
}
console.log(`marca: ${nombre} (identidad ${identidad.slug})`);
