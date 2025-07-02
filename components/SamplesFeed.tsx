'use client';
import { useEffect, useState } from "react";
import { apiFetch } from "../lib/api";
import { isDebug } from "../lib/debug";
import { useAuthStore } from "../store/authStore";
import SampleCard from "./SampleCard";

export default function SamplesFeed() {
  const { token } = useAuthStore();
  const [samples, setSamples] = useState<any[]>([]);
  const [debugInfo, setDebugInfo] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function load() {
      try {
        const endpoint = token ? "/feed" : "/contents";
        const res = await apiFetch<{ data: any[] }>(endpoint, {
          method: "GET",
        });
        // La API devuelve data directamente cuando es listado paginado.
        const list = Array.isArray(res.data) ? res.data : res.data?.data ?? [];
        setSamples(list);
        if (isDebug) setDebugInfo(res);
      } catch (e: any) {
        setError(e.message);
      }
    }
    load();
  }, [token]);

  if (error) return <p>Error: {error}</p>;

  return (
    <section style={{ flex: 1 }}>
      {samples.length === 0 ? (
        <p>No hay samples disponibles.</p>
      ) : (
        <div style={{ display: "grid", gap: "16px" }}>
          {samples.map((s) => (
            <SampleCard key={s.id} sample={s} />
          ))}
        </div>
      )}
      {isDebug && debugInfo && (
        <details style={{ marginTop: "16px" }} open>
          <summary>Debug API response</summary>
          <pre style={{ whiteSpace: "pre-wrap", fontSize: "12px" }}>{JSON.stringify(debugInfo, null, 2)}</pre>
        </details>
      )}
    </section>
  );
} 