// app/components/layout/Header.tsx
'use client';

import InterruptorTema from './InterruptorTema';
import Boton from '@/components/ui/Boton';
import {useAuth} from '@/context/AuthContext';
import SubirSample from '@/components/SubirSample';

export default function Header() {
	const {usuario, logout, cargando} = useAuth();

	return (
		<header className="cabeceraPrincipal">
			{/* El componente de búsqueda irá aquí en el futuro */}
			<div className="espaciador"></div>

			<div className="controlesCabecera">
				<InterruptorTema />
				{cargando ? (
					<div className="cargandoAuth"></div>
				) : usuario ? (
					<>
						<SubirSample />
						<span className="nombreUsuario">Hola, {usuario.nombremostrado}</span>
						<Boton onClick={logout} variante="secundario">
							Salir
						</Boton>
					</>
				) : (
					<>
						<Boton href="/login" variante="secundario">
							Iniciar Sesión
						</Boton>
						<Boton href="/registro" variante="primario">
							Registrarse
						</Boton>
					</>
				)}
			</div>

			<style jsx>{`
				.cabeceraPrincipal {
					display: flex;
					align-items: center;
					justify-content: space-between;
					padding: 1rem 2rem;
					border-bottom: 1px solid var(--color-borde);
					background-color: var(--color-fondo);
				}
				.espaciador {
					flex-grow: 1;
				}
				.controlesCabecera {
					display: flex;
					align-items: center;
					gap: 1rem;
				}
				.nombreUsuario {
					font-weight: 600;
				}
				.cargandoAuth {
					/* Placeholder para un futuro spinner */
					width: 70px;
					height: 24px;
					border-radius: 8px;
					background-color: var(--color-borde);
				}
			`}</style>
		</header>
	);
}