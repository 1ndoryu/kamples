/**
 * Único punto de salida a consola del framework glory-core.
 * - error: siempre (son fallos que el usuario debe poder reportar).
 * - warn / info / debug: solo en desarrollo; en producción no ensucian la consola.
 * Las llamadas directas a console en el resto del framework se consideran defecto.
 */

const esDesarrollo = import.meta.env.DEV;

export const logger = {
  // sentinel-disable-next-line console-production — punto único de salida a consola, ver cabecera
  error: (...args: unknown[]): void => console.error(...args),
  // sentinel-disable-next-line console-production — punto único de salida a consola, ver cabecera
  warn: (...args: unknown[]): void => { if (esDesarrollo) console.warn(...args); },
  // sentinel-disable-next-line console-production — punto único de salida a consola, ver cabecera
  info: (...args: unknown[]): void => { if (esDesarrollo) console.info(...args); },
  // sentinel-disable-next-line console-production — punto único de salida a consola, ver cabecera
  debug: (...args: unknown[]): void => { if (esDesarrollo) console.debug(...args); },
};
