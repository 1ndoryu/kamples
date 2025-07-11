import { apiFetch } from "../lib/api";
import { create } from "zustand";

interface AuthState {
  token: string | null;
  user: any | null;
  login: (identifier: string, password: string) => Promise<void>;
  register: (username: string, email: string, password: string) => Promise<void>;
  logout: () => void;
  hydrate: () => void;
  loadProfile: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set, get) => ({
  // Evitamos leer localStorage durante el SSR para prevenir hydration mismatch.
  token: null,
  user: null,

  async loadProfile() {
    const token = typeof window !== "undefined" ? localStorage.getItem("token") : null;
    if (!token) return; // No hay token -> nada que hacer

    const [res, error] = await apiFetch("/user/profile");

    if (error) {
      console.error("Error cargando perfil de usuario", error);
      localStorage.removeItem("token");
      set({ token: null, user: null });
      return;
    }

    if (res) {
      // La API podría devolver el usuario en res.data o en res.user según versión
      // tenemos que cambiar esto t-t
      const user = (res as any).user ?? (res as any).data ?? null;
      if (user) {
        set({ user });
      }
    }
  },
  async login(identifier, password) {
    const [res, error] = await apiFetch<{ token_type: string; access_token: string; expires_in: number; }>("/auth/login", {
      method: "POST",
      body: JSON.stringify({ identifier, password }),
      skipAuth: true,
    });

    if (error) {
      console.error("Error al iniciar sesión", error);
      localStorage.removeItem("token");
      set({ token: null, user: null });
      return;
    }

    if (res) {
      localStorage.setItem("token", res.data.access_token);
      set({ token: res.data.access_token });
      // Una vez guardado el token, obtenemos el perfil
      await get().loadProfile();
    }
  },
  async register(username, email, password) {
    const [res, error] = await apiFetch<null>("/auth/register", {
      method: "POST",
      body: JSON.stringify({ username, email, password }),
      skipAuth: true,
    });

    if (error) {
      console.error("Error al registrar", error);
      return;
    }

    // Si el proceso de registro hacemos el login de una ve'.
    if (res) {
      await get().login(email, password);
    }
  },
  logout() {
    localStorage.removeItem("token");
    set({ token: null, user: null });
  },
  hydrate() {
    const savedToken = typeof window !== "undefined" ? localStorage.getItem("token") : null;
    if (savedToken) {
      set({ token: savedToken });
      // Cargamos el perfil si no está aún
      get().loadProfile();
    }
  }
})); 