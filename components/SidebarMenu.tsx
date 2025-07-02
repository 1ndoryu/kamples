'use client';
import Link from "next/link";
import styles from "./SidebarMenu.module.css";

export default function SidebarMenu() {
  return (
    <aside className={`${styles.menuLateral} bloque`}>
      <ul>
        <li>
          <Link href="/">Inicio</Link>
        </li>
      </ul>
    </aside>
  );
} 