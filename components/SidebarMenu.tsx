'use client';
import Link from "next/link";
import styles from "./SidebarMenu.module.css";

export default function SidebarMenu() {
  return (
    <aside className={`${styles.menuLateral} bloque`}>
      <ul>
        <li>
          <Link href="/">Todos los samples</Link>
        </li>
        <li>
          <Link href="/?tipo=audio_sample">Audio Samples</Link>
        </li>
        <li>
          <Link href="/?tipo=loop">Loops</Link>
        </li>
      </ul>
    </aside>
  );
} 