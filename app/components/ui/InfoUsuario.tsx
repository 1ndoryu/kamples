// app/components/ui/InfoUsuario.tsx
'use client';

import {useAuth} from '@/context/AuthContext';

export default function InfoUsuario() {
    const {usuario} = useAuth();

    if (!usuario) {
        return null; // O un esqueleto de carga
    }

    // Nota: Asumo que la imagen de perfil estará en `usuario.imagen_perfil`
    // Si no existe, se podría mostrar un placeholder genérico.
    const imagenSrc = (usuario as any).imagen_perfil || `https://2upra.com/wp-content/uploads/2025/01/1ndoryu_1735753838.jpg`; // Placeholder temporal
    const nombreMostrado = usuario.nombremostrado || usuario.nombreusuario;

    return (
        <div className="infoUsuario W8DK25">
            <img src={imagenSrc} alt={`Perfil de ${nombreMostrado}`} />
            <p>{nombreMostrado}</p>
            <style jsx>{`
                .infoUsuario {
                    display: flex;
                    align-items: center;
                    gap: 10px;
                }
                .infoUsuario img {
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
