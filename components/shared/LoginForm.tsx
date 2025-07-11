'use client';
import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useAuthStore } from '../../store/authStore';
import { useUiStore } from '../../store/uiStore';
import styles from './LoginForm.module.css';
import { KamplesLogo } from '@/public/logo/Kamples';

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
    <div className={styles.loginForm}>
      <div className={styles.asideForm}>
          <KamplesLogo width={35} height={35} fill="var(--foreground)" />
      </div>

      <form onSubmit={handleSubmit} className={styles.formBody}>
        <div className={styles.formContainer}>

          <div className={styles.labelContainer}>
            <label htmlFor="identifier" className={styles.label}>
              Email o usuario
            </label>
            <input
              id="identifier"
              className={styles.input}
              type="text"
              value={identifier}
              onChange={(e) => setIdentifier(e.target.value)}
              placeholder=" " 
            />
          </div>

          <div className={styles.labelContainer}>
            <label htmlFor="password" className={styles.label}>
              Contraseña
            </label>
            <input
              id="password"
              className={styles.input}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder=" " 
              />
          </div>

          <button type="submit" className={styles.btnSubmit}>
            Entrar
          </button>
        </div>
      </form>
    </div>
  );
}