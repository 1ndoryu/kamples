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
        </>
    );
}
