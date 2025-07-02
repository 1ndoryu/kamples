'use client';
import Modal from './Modal';
import { useUiStore } from '../store/uiStore';
import LoginForm from './LoginForm';

export default function LoginModal() {
  const modalOpen = useUiStore((s) => s.modalOpen);

  if (modalOpen !== 'login') return null;

  return (
    <Modal>
      <LoginForm />
    </Modal>
  );
} 