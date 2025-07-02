'use client';
import Link from "next/link";
import { useAuthStore } from "@/store/authStore";
import { useEffect } from "react";
import styles from "./TopMenu.module.css";

export default function TopMenu() {
  const { token, logout, hydrate } = useAuthStore();

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
        <Link href="/publicar">Publicar</Link>
      </nav>
      <nav className={styles.navAuth}>
        {token ? (
          <button onClick={logout}>Cerrar sesión</button>
        ) : (
          <>
            <Link href="/login">Entrar</Link>
            <Link href="/registro">Registro</Link>
          </>
        )}
      </nav>
    </header>
  );
} 