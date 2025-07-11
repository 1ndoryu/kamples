'use client';
import Link from "next/link";
import styles from "./SidebarMenu.module.css";
import { useUiStore } from "../store/uiStore";
import { KamplesLogo } from "../public/logo/Kamples";

export default function SidebarMenu() {
  const openModal = useUiStore((s) => s.openModal);

  return (
    <aside className={`${styles.menuLateral}`}>
      <ul>
        <li>
          <Link href="/">
            <KamplesLogo width={24} height={24} fill="var(--foreground)" />
          </Link>
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