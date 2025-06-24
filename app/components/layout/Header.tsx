// app/components/layout/Header.tsx
'use client';

import InterruptorTema from './InterruptorTema';
import Boton from '@/components/ui/Boton';
// import {useAuth} from '@/context/AuthContext'; // Eliminado
import { useAppStore } from '@/store/useAppStore'; // Importar el store de Zustand
import SubirSample from '@/components/SubirSample';

export default function Header() {
	// const {usuario, logout, cargando} = useAuth(); // Eliminado
	const usuario = useAppStore((state) => state.user);
	const logoutStore = useAppStore((state) => state.logoutStore);
	const cargando = useAppStore((state) => state.isLoading);
	const isAuthenticated = useAppStore((state) => state.isAuthenticated);


	const handleLogout = async () => {
		await logoutStore();
		// Aquí podrías añadir redirección si es necesario, por ejemplo:
		// router.push('/');
		// Pero usualmente la redirección después del logout se maneja en la página
		// o el logoutStore podría manejar una redirección global si siempre es la misma.
		// Por ahora, solo se limpia el estado. El router de Next.js no está disponible aquí directamente.
	  };

	return (
		<header className="cabeceraPrincipal">
			{/* El componente de búsqueda irá aquí en el futuro */}
			<div className="espaciador"></div>

			<div className="controlesCabecera">
				<InterruptorTema />
				{cargando ? (
					<div className="cargandoAuth">Cargando...</div> // Mejor mostrar algo
				) : isAuthenticated && usuario ? ( // Usar isAuthenticated y verificar usuario
					<>
						<SubirSample />
						<span className="nombreUsuario">Hola, {usuario.nombremostrado}</span>
						<Boton onClick={handleLogout} variante="secundario"> {/* Usar handleLogout */}
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