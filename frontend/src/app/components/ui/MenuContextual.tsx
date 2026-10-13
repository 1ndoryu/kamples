/* [174A-109b-fase2] Compat wrapper para `@app/components/ui/MenuContextual`.
 * Implementa solo el dropdown desktop que Mezclador necesita para compilar y operar. */

import { Fragment, type ReactNode, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import '../../styles/legacyUi.css';
import { BotonBase } from './BotonBase';
import { anchoVentana, altoVentana } from '../../../glory-core/platform/ventana';
import { cuerpoDocumento } from '../../../glory-core/platform/documento';

export interface MenuItemDef {
  id: string;
  etiqueta: string;
  icono?: ReactNode;
  peligro?: boolean;
  separadorDespues?: boolean;
  href?: string;
  onClick: () => void;
}

interface MenuContextualProps {
  abierto: boolean;
  onCerrar: () => void;
  items: MenuItemDef[];
  x: number;
  y: number;
  alinearDerecha?: boolean;
  forzarDropdown?: boolean;
}

export const MenuContextual = ({
  abierto,
  onCerrar,
  items,
  x,
  y,
  alinearDerecha = false,
}: MenuContextualProps): JSX.Element | null => {
  const menuRef = useRef<HTMLDivElement | null>(null);
  const [position, setPosition] = useState({ left: x, top: y });

  useEffect(() => {
    if (!abierto) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onCerrar();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [abierto, onCerrar]);

  useLayoutEffect(() => {
    if (!abierto) {
      return;
    }

    const node = menuRef.current;
    const margin = 8;
    const width = node?.offsetWidth ?? 180;
    const height = node?.offsetHeight ?? (items.length * 40 + 16);
    const targetLeft = alinearDerecha ? x - width : x;

    setPosition({
      left: Math.max(margin, Math.min(targetLeft, anchoVentana() - width - margin)),
      top: Math.max(margin, Math.min(y, altoVentana() - height - margin)),
    });
  }, [abierto, alinearDerecha, items.length, x, y]);

  if (!abierto) {
    return null;
  }

  const renderedItems = items.map((item) => (
    <Fragment key={item.id}>
      {item.href ? (
        <a
          className={`botonBase menuContextualItem ${item.peligro ? 'itemPeligro' : ''}`.trim()}
          href={item.href}
          onClick={(event) => {
            if (event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey) {
              event.preventDefault();
              item.onClick();
              onCerrar();
            }
          }}
          role="menuitem"
        >
          {item.icono}
          {item.etiqueta}
        </a>
      ) : (
        <BotonBase
          variante="ghost"
          className={`menuContextualItem ${item.peligro ? 'itemPeligro' : ''}`.trim()}
          onClick={() => {
            item.onClick();
            onCerrar();
          }}
          role="menuitem"
          type="button"
        >
          {item.icono}
          {item.etiqueta}
        </BotonBase>
      )}
      {item.separadorDespues && <div className="menuContextualSeparador" />}
    </Fragment>
  ));

  return createPortal(
    <>
      <div className="menuContextualOverlay" onClick={onCerrar} />
      <div ref={menuRef} className="menuContextual" role="menu" style={position}>
        {renderedItems}
      </div>
    </>,
    cuerpoDocumento(),
  );
};

export default MenuContextual;
