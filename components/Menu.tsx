'use client';

import { createContext, useContext, useState, useRef, useEffect, ReactNode } from 'react';
import styles from './Menu.module.css';

interface MenuContextType {
  open: boolean;
  setOpen: (v: boolean) => void;
}

const MenuContext = createContext<MenuContextType | null>(null);

export function MenuContainer({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Cerrar menú al hacer clic fuera
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (!containerRef.current) return;
      if (!containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  return (
    <MenuContext.Provider value={{ open, setOpen }}>
      <div ref={containerRef} style={{ position: 'relative', display: 'inline-block' }}>
        {children}
      </div>
    </MenuContext.Provider>
  );
}

export function MenuButton({ children }: { children: ReactNode }) {
  const ctx = useContext(MenuContext);
  if (!ctx) throw new Error('MenuButton debe usarse dentro de MenuContainer');
  return (
    <button
      type="button"
      onClick={() => ctx.setOpen(!ctx.open)}
      style={{ background: 'transparent', border: 'none', cursor: 'pointer', color: 'inherit', padding: 0, margin: 0}}
    >
      {children}
    </button>
  );
}

export function Menu({ children, width }: { children: ReactNode; width?: number }) {
  const ctx = useContext(MenuContext);
  if (!ctx) throw new Error('Menu debe usarse dentro de MenuContainer');
  if (!ctx.open) return null;
  return (
    <div className={styles.menu} style={width ? { width } : undefined}>
      {children}
    </div>
  );
}

interface MenuItemProps {
  children: ReactNode;
  onClick: () => void;
  type?: 'error' | 'default';
}

export function MenuItem({ children, onClick, type = 'default' }: MenuItemProps) {
  const ctx = useContext(MenuContext);
  if (!ctx) throw new Error('MenuItem debe usarse dentro de MenuContainer');

  const handleClick = () => {
    onClick();
    ctx.setOpen(false);
  };

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={handleClick}
      onKeyDown={(e) => { if (e.key === 'Enter') handleClick(); }}
      className={`${styles.menuItem} ${type === 'error' ? styles.error : ''}`}
    >
      {children}
    </div>
  );
} 