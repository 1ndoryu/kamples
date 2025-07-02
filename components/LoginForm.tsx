'use client';
import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useAuthStore } from '../store/authStore';
import { useUiStore } from '../store/uiStore';

export default function LoginForm() {
  const { login } = useAuthStore();
  const closeModal = useUiStore((s) => s.closeModal);
  const [identifier, setIdentifier] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const router = useRouter();

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    try {
      await login(identifier, password);
      closeModal();
      router.push('/');
    } catch (e: any) {
      setError(e.message);
    }
  }

  return (
    <>
      <h1>Iniciar sesión</h1>
      {error && <p style={{ color: 'red' }}>{error}</p>}
      <form onSubmit={handleSubmit} className="bloque">
        <div className="bloque">
          <label>Email o usuario</label>
          <input value={identifier} onChange={(e) => setIdentifier(e.target.value)} />
        </div>
        <div className="bloque">
          <label>Contraseña</label>
          <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
        </div>
        <button type="submit">Entrar</button>
      </form>
    </>
  );
} 