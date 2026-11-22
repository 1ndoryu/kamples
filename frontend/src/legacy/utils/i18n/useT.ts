/* [183A-111] useT — Hook de traducción.
 * Accede al JSON del idioma activo (es/en/ja) via useIdiomaStore.
 * getT() permite acceso no-reactivo desde servicios/utils. */
import { useIdiomaStore, type Idioma } from './idiomaStore';
import esJson from './es.json';
import enJson from './en.json';
import jaJson from './ja.json';
import { NOMBRE as MARCA } from '../../../marca';

type Libro = Record<string, string>;
type Params = Record<string, string | number>;

const libros: Record<Idioma, Libro> = {
    es: esJson as Libro,
    en: enJson as Libro,
    ja: jaJson as Libro,
};

function resolver(libro: Libro, clave: string, fallback: Libro): string {
    return libro[clave] ?? fallback[clave] ?? clave;
}

/* `marca` se inyecta siempre: los textos que llevan el nombre de la app lo
 * escriben como `{marca}`, y así cambiar la marca no toca los tres idiomas. */
function interpolar(texto: string, params: Params): string {
    const conMarca: Params = { marca: MARCA, ...params };
    return texto.replace(/\{(\w+)\}/g, (_, k) => String(conMarca[k] ?? `{${k}}`));
}

export interface UseTResult {
    t: (clave: string, params?: Params) => string;
    idioma: Idioma;
}

export function useT(): UseTResult {
    const idioma = useIdiomaStore(s => s.idioma);
    const t = (clave: string, params?: Params): string => {
        const texto = resolver(libros[idioma], clave, libros.es);
        return interpolar(texto, params ?? {});
    };
    return { t, idioma };
}

/* Para uso fuera de componentes (servicios, utils) — no reactivo */
export function getT(idioma?: Idioma): (clave: string, params?: Params) => string {
    const i: Idioma = idioma ?? (() => {
        if (typeof localStorage === 'undefined') return 'es';
        const g = localStorage.getItem('kamples_idioma');
        return (g === 'es' || g === 'en' || g === 'ja') ? g : 'es';
    })();
    return (clave, params) => {
        const texto = resolver(libros[i], clave, libros.es);
        return interpolar(texto, params ?? {});
    };
}
