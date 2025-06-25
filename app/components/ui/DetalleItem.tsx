import React from 'react';

interface DetalleItemProps {
    icon: React.ReactNode;
    label: string;
    value: string | number;
}

const DetalleItem: React.FC<DetalleItemProps> = ({ icon, label, value }) => {
    return (
        <div className="detalle-item-container">
            <div className="icon-container">{icon}</div>
            <div className="text-container">
                <span className="label">{label}</span>
                <span className="value">{value}</span>
            </div>
        </div>
    );
};

export default DetalleItem;