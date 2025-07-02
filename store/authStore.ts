import { apiFetch } from "@/lib/api";
import { create } from "zustand";

interface AuthState {
  token: string | null;
  user: any | null;
  login: (identifier: string, password: string) => Promise<void>;
  register: (username: string, email: string, password: string) => Promise<void>;
  logout: () => void;
  hydrate: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  // Evitamos leer localStorage durante el SSR para prevenir hydration mismatch.
  token: null,
  user: null,
  async login(identifier, password) {
    const res = await apiFetch<{
      token_type: string;
      access_token: string;
      expires_in: number;
    }>("/auth/login", {
      method: "POST",
      body: JSON.stringify({ identifier, password }),
      skipAuth: true,
    });

    if (res.success) {
      localStorage.setItem("token", res.data.access_token);
      set({ token: res.data.access_token });
    }
  },
  async register(username, email, password) {
    await apiFetch<null>("/auth/register", {
      method: "POST",
      body: JSON.stringify({ username, email, password }),
      skipAuth: true,
    });
    // Después de registrar, iniciar sesión automáticamente
    await (useAuthStore.getState().login as any)(email, password);
  },
  logout() {
    localStorage.removeItem("token");
    set({ token: null, user: null });
  },
  hydrate() {
    const savedToken = typeof window !== "undefined" ? localStorage.getItem("token") : null;
    if (savedToken) {
      set({ token: savedToken });
    }
  }
})); 