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
			{/* El bloque <style jsx> ha sido movido a LayoutStyles.tsx */}
		</header>
	);
}