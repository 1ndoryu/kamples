'use client';
import Link from "next/link";
import { useAuthStore } from "../store/authStore";
import { useEffect } from "react";
import { useUiStore } from "../store/uiStore";
import styles from "./TopMenu.module.css";

export default function TopMenu() {
  const { token, logout, hydrate } = useAuthStore();
  const openModal = useUiStore((s) => s.openModal);

  // Sincronizamos el token almacenado después de que el componente se monte en el cliente
  useEffect(() => {
    hydrate();
    // solo ejecutar una vez
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <header className={`${styles.menuSuperior} bloque`}>
      <nav className={styles.navLinks}>
        <Link href="/">Inicio</Link>
        <button onClick={() => openModal('publicar')} className="bloque" style={{ background: 'none', border: 'none', color: 'inherit', cursor: 'pointer' }}>Publicar</button>
      </nav>
      <nav className={styles.navAuth}>
        {token ? (
          <button onClick={logout}>Cerrar sesión</button>
        ) : (
          <>
            <button onClick={() => openModal('login')} className="bloque" style={{ background: 'none', border: 'none', color: 'inherit', cursor: 'pointer' }}>Entrar</button>
            <Link href="/registro">Registro</Link>
          </>
        )}
      </nav>
    </header>
  );
} 