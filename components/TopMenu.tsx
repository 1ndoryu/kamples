'use client';
import Link from 'next/link';
import {useAuthStore} from '../store/authStore';
import {useEffect, useState, useRef} from 'react';
import {useUiStore} from '../store/uiStore';
import { useSearchStore } from '../store/searchStore';
import { apiFetch } from '../lib/api';
import styles from './TopMenu.module.css';
import Avatar from './Avatar';
import {MenuContainer, MenuButton, Menu, MenuItem} from './Menu';

export default function TopMenu() {
    const {token, user, logout, hydrate} = useAuthStore();
    const openModal = useUiStore(s => s.openModal);

    const { searchTerm, setSearchTerm } = useSearchStore();
    const [inputValue, setInputValue] = useState('');
    const [previewResults, setPreviewResults] = useState<any[]>([]);
    const [showPreview, setShowPreview] = useState(false);
    const debounceRef = useRef<number>();

    useEffect(() => {
        hydrate();
    }, []);

    // ---------- búsqueda en tiempo real ----------
    useEffect(() => {
        // limpiamos debounce anterior
        if (debounceRef.current) {
            clearTimeout(debounceRef.current);
        }

        // si input vacío, limpiamos resultados
        if (inputValue.trim() === '') {
            setPreviewResults([]);
            setShowPreview(false);
            return;
        }

        debounceRef.current = window.setTimeout(async () => {
            try {
                const res = await apiFetch<any>(`/search?q=${encodeURIComponent(inputValue)}&user_id=${user?.id ?? 0}&page=1&per_page=5`, { method: 'GET' });
                const ids: number[] = res.data?.sample_ids ?? res.data?.data ?? [];
                if (!Array.isArray(ids) || ids.length === 0) {
                    setPreviewResults([]);
                    setShowPreview(false);
                    return;
                }

                // Cargamos detalles de cada sample (máx 5) usando /admin/contents/{id}
                const detailsPromises = ids.slice(0, 5).map((id: number) => apiFetch<any>(`/admin/contents/${id}`, { method: 'GET' }).then(r => r.data));
                const details = await Promise.all(detailsPromises);
                setPreviewResults(details);
                setShowPreview(true);
            } catch (e) {
                console.error('Error buscando', e);
            }
        }, 400); // debounce 400ms

        // cleanup
        return () => {
            if (debounceRef.current) clearTimeout(debounceRef.current);
        };
    }, [inputValue, user]);

    // Cerrar preview al hacer click fuera
    useEffect(() => {
        const handler = (e: MouseEvent) => {
            if (!(e.target as HTMLElement).closest('#inputBusqueda')) {
                setShowPreview(false);
            }
        };
        document.addEventListener('click', handler);
        return () => document.removeEventListener('click', handler);
    }, []);

    const confirmarBusqueda = () => {
        setSearchTerm(inputValue.trim());
        setShowPreview(false);
    };

    return (
        <header className={`${styles.menuSuperior}`}>
            <nav className={styles.navLinks} style={{ display: 'flex', alignItems: 'center', position: 'relative' }}>
                {/* Input de búsqueda */}
                <input
                    id="inputBusqueda"
                    type="text"
                    placeholder="Buscar..."
                    value={inputValue}
                    onChange={(e) => setInputValue(e.target.value)}
                    onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                            confirmarBusqueda();
                        }
                    }}
                    className="bloque"
                    style={{ padding: '4px 8px' }}
                />

                {/* Vista previa resultados */}
                {showPreview && previewResults.length > 0 && (
                    <div style={{ position: 'absolute', top: '100%', left: 0, right: 0, background: '#222', border: '1px solid #444', zIndex: 20 }}>
                        {previewResults.map((item: any) => (
                            <div
                                key={item.id}
                                onClick={() => {
                                    setInputValue(item.content_data?.title ?? '');
                                    confirmarBusqueda();
                                }}
                                style={{ padding: '6px 8px', cursor: 'pointer' }}
                                className="bloque"
                            >
                                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                                    {item.content_data?.cover_url && (
                                        <img src={item.content_data.cover_url} alt={item.content_data.title} style={{ width: '32px', height: '32px', objectFit: 'cover' }} />
                                    )}
                                    <div style={{ display: 'flex', flexDirection: 'column' }}>
                                        <span>{item.content_data?.title ?? 'Sin título'}</span>
                                        <small style={{ opacity: 0.7 }}>sample</small>
                                    </div>
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </nav>
            <nav className={styles.navAuth}>
                {token ? (
                    <>
                        <button onClick={() => openModal('publicar')} className="" style={{background: 'none', border: 'none', color: 'inherit', cursor: 'pointer', margin: 0, padding: 0}}>
                            <svg data-testid="geist-icon" height="16" strokeLinejoin="round" viewBox="0 0 16 16" width="16" style={{color: 'currentcolor'}}>
                                <path fillRule="evenodd" clipRule="evenodd" d="M14.5 8C14.5 11.5899 11.5899 14.5 8 14.5C4.41015 14.5 1.5 11.5899 1.5 8C1.5 4.41015 4.41015 1.5 8 1.5C11.5899 1.5 14.5 4.41015 14.5 8ZM16 8C16 12.4183 12.4183 16 8 16C3.58172 16 0 12.4183 0 8C0 3.58172 3.58172 0 8 0C12.4183 0 16 3.58172 16 8ZM8.75 4.25V5V7.25H11H11.75V8.75H11H8.75V11V11.75L7.25 11.75V11V8.75H5H4.25V7.25H5H7.25V5V4.25H8.75Z" fill="currentColor" />
                            </svg>
                        </button>
                        <svg data-testid="geist-icon" height="16" strokeLinejoin="round" viewBox="0 0 16 16" width="16" style={{color: 'currentcolor'}}>
                            <path fillRule="evenodd" clipRule="evenodd" d="M2.8914 10.4028L2.98327 10.6318C3.22909 11.2445 3.5 12.1045 3.5 13C3.5 13.3588 3.4564 13.7131 3.38773 14.0495C3.69637 13.9446 4.01409 13.8159 4.32918 13.6584C4.87888 13.3835 5.33961 13.0611 5.70994 12.7521L6.22471 12.3226L6.88809 12.4196C7.24851 12.4724 7.61994 12.5 8 12.5C11.7843 12.5 14.5 9.85569 14.5 7C14.5 4.14431 11.7843 1.5 8 1.5C4.21574 1.5 1.5 4.14431 1.5 7C1.5 8.18175 1.94229 9.29322 2.73103 10.2153L2.8914 10.4028ZM2.8135 15.7653C1.76096 16 1 16 1 16C1 16 1.43322 15.3097 1.72937 14.4367C1.88317 13.9834 2 13.4808 2 13C2 12.3826 1.80733 11.7292 1.59114 11.1903C0.591845 10.0221 0 8.57152 0 7C0 3.13401 3.58172 0 8 0C12.4183 0 16 3.13401 16 7C16 10.866 12.4183 14 8 14C7.54721 14 7.10321 13.9671 6.67094 13.9038C6.22579 14.2753 5.66881 14.6656 5 15C4.23366 15.3832 3.46733 15.6195 2.8135 15.7653Z" fill="currentColor" />
                        </svg>
                        <MenuContainer>
                            <MenuButton>{user && <Avatar avatarUrl={user.profile_data?.avatar_url ?? null} size={32} />}</MenuButton>
                            <Menu width={160}>
                                <MenuItem onClick={logout} type="error">
                                    Cerrar sesión
                                </MenuItem>
                            </Menu>
                        </MenuContainer>
                    </>
                ) : (
                    <>
                        <button onClick={() => openModal('login')} className="bloque" style={{background: 'none', border: 'none', color: 'inherit', cursor: 'pointer'}}>
                            Entrar
                        </button>
                        <Link href="/registro">Registro</Link>
                    </>
                )}
            </nav>
        </header>
    );
}
