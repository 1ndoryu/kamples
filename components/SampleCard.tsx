'use client';

import styles from "./SampleCard.module.css";
import { isDebug } from "../lib/debug";
import { useState } from "react";

interface Props {
  sample: any;
}

export default function SampleCard({ sample }: Props) {
  const [expandido, setExpandido] = useState(false);

  const renderDebugInfo = () => {
    // Si no hay datos o no estamos en modo debug, no renderizar nada
    if (!isDebug || !sample.content_data) return null;

    // Convertir el objeto en pares clave / valor ordenados por clave
    const entradas = Object.entries(sample.content_data).sort((a, b) =>
      a[0].localeCompare(b[0])
    );

    // Mostrar solo las primeras 5 entradas si no está expandido
    const entradasMostradas = expandido ? entradas : entradas.slice(0, 5);

    return (
      <div className={styles.debugInfo}>
        <ul>
          {entradasMostradas.map(([clave, valor]) => (
            <li key={clave}>
              <strong>{clave}:</strong>{" "}
              {Array.isArray(valor) ? valor.join(", ") : String(valor)}
            </li>
          ))}
        </ul>
        {entradas.length > 5 && (
          <button
            type="button"
            onClick={() => setExpandido(!expandido)}
            className={styles.botonExpandir}
          >
            {expandido ? "Ocultar" : "Expandir"}
          </button>
        )}
      </div>
    );
  };

  return (
    <article className={`${styles.tarjetaSample} bloque`}>
      <h3>{sample.content_data?.title ?? sample.slug}</h3>
      <p>Tipo: {sample.type}</p>
      <p>Publicado por usuario #{sample.user_id}</p>
      {/* Aquí se podría integrar un reproductor de audio más adelante */}
      {renderDebugInfo()}
    </article>
  );
} 