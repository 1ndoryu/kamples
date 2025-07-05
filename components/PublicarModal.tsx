'use client';
import Modal from './Modal';
import {useUiStore} from '../store/uiStore';
import PublicarForm from './PublicarForm';

export default function PublicarModal() {
    const modalOpen = useUiStore(s => s.modalOpen);

    if (modalOpen !== 'publicar') return null;

    return (
        <Modal>
            <PublicarForm />
        </Modal>
    );
}
