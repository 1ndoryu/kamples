/* [204A-1] Stub web para todos los plugins de Tauri usados por el legacy.
 * Solo se importan dinamicamente cuando la app corre en desktop nativo.
 * En web nunca se invocan estos exports, pero Rollup necesita resolverlos. */

const noop = async () => undefined;

export const writeFile = noop;
export const readFile = noop;
export const readTextFile = noop;
export const writeTextFile = noop;
export const exists = async () => false;
export const mkdir = noop;
export const remove = noop;
export const BaseDirectory = { Document: 0, Download: 1, Cache: 2 } as const;

export const open = noop;
/* [18AA-2] `plugin-opener` sustituye al `plugin-shell` deprecado. */
export const openUrl = noop;
class CommandStub {
  static create(): CommandStub { return new CommandStub(); }
  async execute() { return { stdout: '', stderr: '', code: 0 }; }
}
export const Command = CommandStub;

export const isPermissionGranted = async () => false;
export const requestPermission = async () => 'denied' as const;
export const sendNotification = noop;

export default {
    writeFile, readFile, readTextFile, writeTextFile, exists, mkdir, remove, BaseDirectory,
    open, openUrl, Command, isPermissionGranted, requestPermission, sendNotification,
};
