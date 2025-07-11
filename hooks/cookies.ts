interface CookieOptions {
  days?: number;
  path?: string;
  domain?: string;
  secure?: boolean;
  sameSite?: 'Strict' | 'Lax' | 'None';
}

/**
 * Establece o actualiza una cookie en el navegador.
 * Es seguro para usar en SSR (no hará nada en el servidor).
 * Serializa automáticamente los objetos a JSON.
 *
 * @param key La clave de la cookie.
 * @param value El valor a guardar. Puede ser cualquier tipo serializable a JSON.
 * @param options Opciones adicionales para la cookie (días de expiración, path, etc.).
 */
export const setCookie = <T>(key: string, value: T, options: CookieOptions = {}): void => {

  if (typeof document === 'undefined') {
    return;
  }

  const serializedValue = JSON.stringify(value);
  let cookieString = `${encodeURIComponent(key)}=${encodeURIComponent(serializedValue)}`;

  if (options.days) {
    const date = new Date();
    date.setTime(date.getTime() + options.days * 24 * 60 * 60 * 1000);
    cookieString += `; expires=${date.toUTCString()}`;
  }

  cookieString += `; path=${options.path ?? '/'}`;

  if (options.domain) {
    cookieString += `; domain=${options.domain}`;
  }
  if (options.secure) {
    cookieString += `; secure`;
  }
  if (options.sameSite) {
    cookieString += `; samesite=${options.sameSite}`;
  }

  document.cookie = cookieString;
}

/**
 * Lee el valor de una cookie del navegador.
 * Es seguro para usar en SSR (devolverá null en el servidor).
 * Solo es apta para el uso desde el cliente.
 * Deserializa automáticamente los valores JSON.
 *
 * @param key La clave de la cookie a leer.
 * @returns El valor de la cookie parseado, o null si no se encuentra.
 */
export const getCookie = <T>(key: string): T | null => {
  if (typeof document === 'undefined') {
    return null;
  }

  const cookies = document.cookie.split(';').reduce((acc, cookie) => {
    const [cookieKey, cookieValue] = cookie.trim().split('=');
    acc[decodeURIComponent(cookieKey)] = decodeURIComponent(cookieValue);
    return acc;
  }, {} as Record<string, string>);

  const cookieValue = cookies[key];

  if (cookieValue) {
    try {
      return JSON.parse(cookieValue) as T;
    } catch (e) {
      return cookieValue as unknown as T;
    }
  }

  return null;
}

/**
 * Elimina una cookie del navegador.
 * Lo hace estableciendo su fecha de expiración en el pasado.
 *
 * @param key La clave de la cookie a eliminar.
 * @param options Opciones como `path` y `domain` que deben coincidir con las de la cookie original.
 */
export const deleteCookie = (key: string, options: { path?: string; domain?: string } = {}): void => {
  setCookie(key, '', { ...options, days: -1 });
}