'use client';
import Link from "next/link";
import { useAuthStore } from "../store/authStore";
import { useEffect } from "react";
import { useUiStore } from "../store/uiStore";
import styles from "./TopMenu.module.css";
import Avatar from "./Avatar";

export default function TopMenu() {
  const { token, user, logout, hydrate } = useAuthStore();
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
        <button onClick={() => openModal('publicar')} className="" style={{ background: 'none', border: 'none', color: 'inherit', cursor: 'pointer' }}>Publicar</button>
      </nav>
      <nav className={styles.navAuth}>
        {token ? (
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            {user && (
              <Avatar avatarUrl={user.profile_data?.avatar_url ?? null} size={32} />
            )}
            <button onClick={logout}>Cerrar sesión</button>
          </div>
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