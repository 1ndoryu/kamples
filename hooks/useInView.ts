'use client';

import { useEffect, useRef, useState } from 'react';

/**
 * Hook sencillo que indica si el elemento referenciado está visible en el viewport.
 * Devuelve una tupla [ref, inView]. Una vez que el elemento entra en vista por primera vez, inView se queda en true.
 */
export function useInView<T extends HTMLElement>(options?: IntersectionObserverInit): [React.RefObject<T>, boolean] {
  const ref = useRef<T>(null);
  const [inView, setInView] = useState(false);

  useEffect(() => {
    const node = ref.current;
    if (!node) return;

    const observer = new IntersectionObserver((entries, obs) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          setInView(true);
          obs.unobserve(entry.target); // Dejar de observar tras la primera intersección
        }
      });
    }, options);

    observer.observe(node);

    return () => observer.disconnect();
  }, [options]);

  return [ref, inView];
} 