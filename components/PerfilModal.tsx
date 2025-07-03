'use client';
import Modal from './Modal';
import { useUiStore } from '../store/uiStore';
import PerfilForm from './PerfilForm';

export default function PerfilModal() {
  const modalOpen = useUiStore((s) => s.modalOpen);

  if (modalOpen !== 'perfil') return null;

  return (
    <Modal>
      <PerfilForm />
    </Modal>
  );
} 