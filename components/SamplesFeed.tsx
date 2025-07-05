'use client';
import { useEffect, useRef, useState, useCallback } from "react";
import { apiFetch } from "../lib/api";
import { isDebug } from "../lib/debug";
import { useAuthStore } from "../store/authStore";
import { useSearchStore } from "../store/searchStore";
import SampleCard from "./SampleCard";

export default function SamplesFeed() {
  const { token, hydrate } = useAuthStore();
  const { searchTerm } = useSearchStore();
  const [isHydrated, setIsHydrated] = useState(false);
  const [samples, setSamples] = useState<any[]>([]);
  const [debugInfo, setDebugInfo] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  // Paginación
  const [currentPage, setCurrentPage] = useState(1);
  const [lastPage, setLastPage] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  // Ref para el sentinel de scroll infinito
  const loaderRef = useRef<HTMLDivElement | null>(null);

  // Hidratamos el estado de autenticación una sola vez en el cliente
  useEffect(() => {
    if (typeof window === "undefined") return; // Evitamos SSR
    hydrate();
    setIsHydrated(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ----- Función auxiliar para parsear la paginación de la API -----
  const parsePagination = (resData: any) => {
    // Caso /feed -> data.pagination
    if (resData?.pagination) {
      return {
        current: resData.pagination.current_page ?? 1,
        last: resData.pagination.last_page ?? 1,
      };
    }
    // Caso /contents -> current_page / last_page en raiz
    if (typeof resData?.current_page !== "undefined") {
      return {
        current: resData.current_page ?? 1,
        last: resData.last_page ?? 1,
      };
    }
    // Si no hay info de paginación, asumimos una sola página
    return { current: 1, last: 1 };
  };

  const fetchPage = useCallback(
    async (pageToLoad: number) => {
      try {
        setIsLoading(true);
        let endpoint = "";
        if (searchTerm.trim() !== "") {
          const userId = (useAuthStore.getState().user as any)?.id ?? 0;
          endpoint = `/search?q=${encodeURIComponent(searchTerm)}&user_id=${userId}&page=${pageToLoad}&per_page=15`;
        } else {
          const endpointBase = token ? "/feed" : "/contents";
          endpoint = `${endpointBase}?page=${pageToLoad}&per_page=15`;
        }
        const res = await apiFetch<any>(endpoint, { method: "GET" });

        let list: any[] = [];
        let current = 1;
        let last = 1;

        if (searchTerm.trim() !== "") {
          // Respuesta con sample_ids + pagination
          const ids: number[] = res.data?.sample_ids ?? [];
          const paginationInfo = res.data?.pagination ?? {};
          current = paginationInfo.current_page ?? pageToLoad;
          last = paginationInfo.last_page ?? pageToLoad;

          // Cargamos detalles de cada id en paralelo
          const detailPromises = ids.map((id: number) => apiFetch<any>(`/admin/contents/${id}`, { method: "GET" }).then(r => r.data));
          list = await Promise.all(detailPromises);
        } else {
          // Extraemos lista y paginación como antes
          list = Array.isArray(res.data)
            ? res.data
            : res.data?.data ?? [];

          const parsed = parsePagination(res.data);
          current = parsed.current;
          last = parsed.last;
        }

        // Añadimos sin duplicar
        setSamples((prev) => {
          const existingIds = new Set(prev.map((s) => s.id));
          const merged = [...prev];
          list.forEach((item: any) => {
            if (!existingIds.has(item.id)) merged.push(item);
          });
          return merged;
        });

        setCurrentPage(current);
        setLastPage(last);

        if (isDebug) setDebugInfo(res);
      } catch (e: any) {
        setError(e.message);
      } finally {
        setIsLoading(false);
      }
    },
    [token, searchTerm]
  );

  // Primera carga & cuando cambia la búsqueda
  useEffect(() => {
    if (!isHydrated) return;
    // Reiniciamos lista cuando cambia búsqueda
    setSamples([]);
    setCurrentPage(1);
    setLastPage(null);
    fetchPage(1);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isHydrated, token, searchTerm]);

  // Observador para el scroll infinito
  useEffect(() => {
    if (!loaderRef.current) return;
    const options = { root: null, rootMargin: "0px", threshold: 0.1 };
    const observer = new IntersectionObserver((entries) => {
      const entry = entries[0];
      if (entry.isIntersecting && !isLoading) {
        const nextPage = currentPage + 1;
        if (lastPage === null || nextPage <= lastPage) {
          fetchPage(nextPage);
        }
      }
    }, options);

    observer.observe(loaderRef.current);
    return () => observer.disconnect();
  }, [currentPage, lastPage, isLoading, fetchPage]);

  if (error) return <p>Error: {error}</p>;

  return (
    <section style={{ flex: 1 }}>
      {samples.length === 0 ? (
        <p>No hay samples disponibles.</p>
      ) : (
        <div style={{ display: "grid", gap: "10px" }}>
          {samples.map((s) => (
            <SampleCard
              key={s.id}
              sample={s}
              onDeleted={(id) => setSamples((prev) => prev.filter((item) => item.id !== id))}
            />
          ))}
        </div>
      )}
      {/* Loader / sentinel para scroll infinito */}
      <div ref={loaderRef} style={{ height: "1px" }} />

      {isDebug && debugInfo && (
        <details style={{ marginTop: "16px" }} open>
          <summary>Debug API response</summary>
          <pre style={{ whiteSpace: "pre-wrap", fontSize: "12px" }}>{JSON.stringify(debugInfo, null, 2)}</pre>
        </details>
      )}
    </section>
  );
} 