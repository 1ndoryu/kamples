// app/components/ui/Input.tsx
'use client';

interface Props extends React.InputHTMLAttributes<HTMLInputElement> {
    id: string;
    label: string;
}

export default function Input({id, label, ...props}: Props) {
    return (
        <>
            <div className="campoFormulario">
                <label htmlFor={id}>{label}</label>
                <input id={id} {...props} />
            </div>
            <style jsx>{`
                .campoFormulario {
                    margin-bottom: 1.5rem;
                }
                .campoFormulario label {
                    display: block;
                    margin-bottom: 0.5rem;
                    font-weight: 600;
                    color: var(--color-texto);
                }
                .campoFormulario input {
                    width: 100%;
                    padding: 0.75rem;
                    border-radius: 8px;
                    border: 1px solid var(--color-borde);
                    background-color: var(--color-fondo);
                    color: var(--color-texto);
                    font-size: 1rem;
                    transition: border-color 0.2s, box-shadow 0.2s;
                }
                .campoFormulario input:focus {
                    outline: none;
                    border-color: var(--color-primario);
                    box-shadow: 0 0 0 2px var(--color-primario-transparente); /* Un resplandor sutil */
                }
                .campoFormulario input:disabled {
                    background-color: var(--color-fondo-secundario);
                    opacity: 0.6;
                    cursor: not-allowed;
                }
            `}</style>
        </>
    );
}
