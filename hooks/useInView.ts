'use client';

import { useState, useEffect, useCallback } from 'react';

/**
 * Hook que indica si un elemento está visible en el viewport usando un callback ref para una inferencia de tipos automática.
 * 
 * @param options - Opciones para el IntersectionObserver.
 * @returns Una tupla `[ref, inView]`. 
 *   - `ref`: Un callback ref que debes asignar al atributo `ref` de tu elemento JSX.
 *   - `inView`: Un booleano que se vuelve `true` cuando el elemento entra en el viewport y se mantiene así.
 */
export function useInView<T extends HTMLElement>(
  options?: IntersectionObserverInit
): [(node: T | null) => void, boolean] {
  const [node, setNode] = useState<T | null>(null);
  const [inView, setInView] = useState(false);

  const ref = useCallback((node: T | null) => {
    setNode(node);
  }, []);

  useEffect(() => {
    if (!node) {
      return;
    }

    const observer = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting) {
        setInView(true);
        observer.disconnect();
      }
    }, options);

    observer.observe(node);

    return () => observer.disconnect();

  }, [node, options]);

  return [ref, inView];
}

/**
 * 
 * Ejemplo de uso:
 * 
 * import { useInView } from './useInView';
 * 
 * function MyComponent() {

  // El threshold es el porcentaje de visibilidad que tiene que tener el elemento para que se active el callback.
  const [myRef, isInView] = useInView({
    threshold: 0.5,
  });

  return (
    <div>
      <p>Desplázate hacia abajo...</p>
      <div style={{ height: '100vh' }}></div>
      
      Simplemente asigna el `myRef` devuelto.
      TypeScript sabe que es un <div>, por lo que infiere que T es HTMLDivElement.
      <div 
        ref={myRef} 
        style={{
          height: '200px',
          width: '200px',
          backgroundColor: isInView ? 'lightgreen' : 'lightcoral',
          transition: 'background-color 0.5s'
        }}
      >
        {isInView ? '¡Estoy a la vista!' : 'Todavía no me ves...'}
      </div>
    </div>
  );
}
*/