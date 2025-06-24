// app/components/ui/InfoUsuario.tsx
'use client';

// import {useAuth} from '@/context/AuthContext'; // Eliminado
import { useAppStore } from '@/store/useAppStore'; // Importar el store de Zustand
import ImagenSample from './ImagenSample';

export default function InfoUsuario() {
	// const {usuario} = useAuth(); // Eliminado
	const usuario = useAppStore((state) => state.user);
	const isAuthenticated = useAppStore((state) => state.isAuthenticated);

	// Podrías también usar el selector useCurrentUser() si lo prefieres
	// import { useCurrentUser } from '@/store/useAppStore';
	// const usuario = useCurrentUser();

	if (!isAuthenticated || !usuario) { // Comprobar isAuthenticated también
		return null; // O un esqueleto de carga
	}

	const nombreMostrado = usuario.nombremostrado || usuario.nombreusuario; // Los nombres de campo ya coinciden

	return (
		<div className="infoUsuario">
			{/* CAMBIO: Se utiliza ImagenSample para mostrar el avatar o el gradiente */}
			<ImagenSample src={usuario.imagen_perfil} alt={`Perfil de ${nombreMostrado}`} nombre={nombreMostrado} tamaño={35} radio={50} />
			<p>{nombreMostrado}</p>

			{/* El CSS se mantiene igual, las clases se aplican correctamente */}
			<style jsx>{`
				.infoUsuario {
					display: flex;
					align-items: center;
					gap: 10px;
				}
				.infoUsuario :global(.imagenSample) {
					// Apuntamos al componente ImagenSample dentro de este scope
					width: 35px;
					height: 35px;
					border-radius: 50%;
					object-fit: cover;
				}
				.infoUsuario p {
					margin: 0;
					font-size: 0.8rem;
					font-weight: 600;
				}
			`}</style>
		</div>
	);
}