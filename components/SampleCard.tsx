'use client';

import styles from "./SampleCard.module.css";

interface Props {
  sample: any;
}

export default function SampleCard({ sample }: Props) {
  return (
    <article className={`${styles.tarjetaSample} bloque`}>
      <h3>{sample.content_data?.title ?? sample.slug}</h3>
      <p>Tipo: {sample.type}</p>
      <p>Publicado por usuario #{sample.user_id}</p>
      {/* Aquí se podría integrar un reproductor de audio más adelante */}
    </article>
  );
} 