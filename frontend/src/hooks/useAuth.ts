/* [174A-106] Hook useAuth contra el backend Axum.
 *
 * Reemplaza al `useAuth` del legado (que mezclaba cookies WordPress
 * con localStorage). Ahora todo va contra `/api/auth/{login,logout,refresh,register}`
 * y `/api/users/me` usando los hooks generados por Orval.
 *
 * Convención de tokens:
 *   - access token  → localStorage.token   (lo lee el axios-instance)
 *   - refresh token → localStorage.refresh
 *
 * Si en el futuro migramos a cookies HttpOnly, este hook es el único
 * lugar que cambia (axios-instance ya no leería de localStorage).
 */

import { useCallback, useMemo } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import {
  useGoogleLogin,
  useLogin,
  useLogout,
  useRegister,
} from '../api/generated/auth/auth';
import { getMeQueryKey, useMe } from '../api/generated/users/users';
import type {
  AuthResponse,
  ErrorResponse,
  GoogleAuthRequest,
  LoginRequest,
  PrivateProfileResponse,
  RegisterRequest,
} from '../api/generated/model';
import { useGoogleAuth } from './useGoogleAuth';

const TOKEN_KEY = 'token';
const REFRESH_KEY = 'refresh';

function persistTokens(payload: AuthResponse | undefined) {
  if (!payload) return;
  if (payload.token) localStorage.setItem(TOKEN_KEY, payload.token);
  if (payload.refresh_token) localStorage.setItem(REFRESH_KEY, payload.refresh_token);
}

function clearTokens() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(REFRESH_KEY);
}

function unwrap<T>(response: unknown): T | undefined {
  const payload = response as { data?: unknown } | undefined;
  return (payload?.data ?? payload) as T | undefined;
}

function getErrorMessage(error: unknown): string | null {
  const payload = error as { message?: string; error?: string } | null;
  if (!payload) {
    return null;
  }

  if (typeof payload.message === 'string' && payload.message.trim()) {
    return payload.message;
  }

  if (typeof payload.error === 'string' && payload.error.trim()) {
    return payload.error;
  }

  return null;
}

export function useAuth() {
  const queryClient = useQueryClient();
  const hasToken = typeof window !== 'undefined' && !!localStorage.getItem(TOKEN_KEY);

  const meQuery = useMe({
    query: { enabled: hasToken, retry: false, refetchOnWindowFocus: false },
  });
  const loginMutation = useLogin();
  const googleLoginMutation = useGoogleLogin();
  const logoutMutation = useLogout();
  const registerMutation = useRegister();

  const login = useCallback(
    async (body: LoginRequest) => {
      const res = await loginMutation.mutateAsync({ data: body });
      persistTokens(unwrap<AuthResponse>(res));
      await queryClient.invalidateQueries({ queryKey: getMeQueryKey() });
    },
    [loginMutation, queryClient],
  );

  const register = useCallback(
    async (body: RegisterRequest) => {
      const res = await registerMutation.mutateAsync({ data: body });
      persistTokens(unwrap<AuthResponse>(res));
      await queryClient.invalidateQueries({ queryKey: getMeQueryKey() });
    },
    [registerMutation, queryClient],
  );

  const loginWithGoogle = useCallback(
    async (body: GoogleAuthRequest) => {
      const res = await googleLoginMutation.mutateAsync({ data: body });
      persistTokens(unwrap<AuthResponse>(res));
      await queryClient.invalidateQueries({ queryKey: getMeQueryKey() });
    },
    [googleLoginMutation, queryClient],
  );

  const handleGoogleCredential = useCallback(
    async (credential: string) => {
      await loginWithGoogle({ id_token: credential });
    },
    [loginWithGoogle],
  );

  const { buttonContainerRef: googleButtonRef } = useGoogleAuth(handleGoogleCredential, false);

  const logout = useCallback(async () => {
    const refreshToken = localStorage.getItem(REFRESH_KEY) ?? '';
    try {
      await logoutMutation.mutateAsync({ data: { refresh_token: refreshToken } });
    } catch {
      /* Ignoramos errores: el cleanup local debe ejecutarse igual. */
    }
    clearTokens();
    queryClient.removeQueries({ queryKey: getMeQueryKey() });
  }, [logoutMutation, queryClient]);

  const errorMessage = useMemo(() => {
    return (
      getErrorMessage(loginMutation.error as ErrorResponse | null)
      ?? getErrorMessage(registerMutation.error as ErrorResponse | null)
      ?? getErrorMessage(googleLoginMutation.error as ErrorResponse | null)
      ?? getErrorMessage(logoutMutation.error as ErrorResponse | null)
      ?? getErrorMessage(meQuery.error)
    );
  }, [googleLoginMutation.error, loginMutation.error, logoutMutation.error, meQuery.error, registerMutation.error]);

  return {
    user: unwrap<PrivateProfileResponse>(meQuery.data),
    isAuthenticated: hasToken && meQuery.isSuccess,
    isLoading: meQuery.isLoading,
    isSubmitting:
      loginMutation.isPending || registerMutation.isPending || googleLoginMutation.isPending || logoutMutation.isPending,
    login,
    iniciarSesion: (identifier: string, password: string) => login({ identifier, password }),
    register,
    registrar: (body: { nombreVisible: string; username: string; email: string; password: string }) => register({
      nombre_visible: body.nombreVisible || undefined,
      username: body.username,
      email: body.email,
      password: body.password,
    }),
    googleBotonRef: googleButtonRef,
    esGoogleNativo: false,
    loginGoogleNativo: () => Promise.resolve(),
    cargando:
      loginMutation.isPending || registerMutation.isPending || googleLoginMutation.isPending,
    logout,
    errorMessage,
    error:
      loginMutation.error ?? registerMutation.error ?? googleLoginMutation.error ?? logoutMutation.error ?? meQuery.error,
  };
}
