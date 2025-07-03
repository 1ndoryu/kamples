'use client';
import Link from "next/link";
import styles from "./SidebarMenu.module.css";
import { useUiStore } from "../store/uiStore";

export default function SidebarMenu() {
  const openModal = useUiStore((s) => s.openModal);

  return (
    <aside className={`${styles.menuLateral} bloque`}>
      <ul>
        <li>
          <Link href="/">Inicio</Link>
        </li>
        <li>
          <button type="button" onClick={() => openModal('perfil')}>
            Editar perfil
          </button>
        </li>
      </ul>
    </aside>
  );
} 