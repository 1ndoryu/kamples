'use client';
import { ReactNode, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { useUiStore } from '../store/uiStore';

interface ModalProps {
  children: ReactNode;
}

const overlayStyle: React.CSSProperties = {
  position: 'fixed',
  top: 0,
  left: 0,
  width: '100vw',
  height: '100vh',
  backgroundColor: 'rgba(0,0,0,0.6)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  zIndex: 1000,
};

const modalStyle: React.CSSProperties = {
  backgroundColor: '#1a1a1a',
  padding: '24px',
  borderRadius: '8px',
  maxWidth: '600px',
  width: '90%',
  maxHeight: '90vh',
  overflowY: 'auto',
};

export default function Modal({ children }: ModalProps) {
  const closeModal = useUiStore((s) => s.closeModal);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') closeModal();
    }
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [closeModal]);

  // Evitamos SSR
  if (typeof window === 'undefined') return null;

  return createPortal(
    <div style={overlayStyle} onClick={closeModal}>
      <div style={modalStyle} onClick={(e) => e.stopPropagation()} className="bloque">
        {children}
      </div>
    </div>,
    document.body
  );
} 