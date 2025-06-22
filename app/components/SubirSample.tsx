// app/components/SubirSample.tsx
'use client';

import { useState } from 'react';
import Boton from '@/components/ui/Boton';
import Modal from '@/components/ui/Modal';
import FormularioSubirSample from './forms/FormularioSubirSample';

export default function SubirSample() {
    const [modalAbierto, setModalAbierto] = useState(false);

    return (
        <>
            <Boton onClick={() => setModalAbierto(true)} variante="secundario">
                Subir Sample
            </Boton>

            <Modal
                titulo="Subir Nuevo Sample"
                estaAbierto={modalAbierto}
                alCerrar={() => setModalAbierto(false)}
            >
                <FormularioSubirSample alCerrar={() => setModalAbierto(false)} />
            </Modal>
        </>
    );
}