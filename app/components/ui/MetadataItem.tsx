import React from 'react';

interface MetadataItemProps {
    label: string;
    value: string | number;
}

const MetadataItem: React.FC<MetadataItemProps> = ({ label, value }) => {
    return (
        <div className="metadata-item bg-white/5 dark:bg-black/10 p-4 rounded-lg transition-all duration-300 hover:bg-white/10 dark:hover:bg-black/20 shadow-md hover:shadow-lg">
            <span className="block text-sm font-medium text-gray-500 dark:text-gray-400 capitalize">{label.replace(/_/g, ' ')}</span>
            <span className="block text-xl font-semibold text-gray-800 dark:text-white mt-1">{value}</span>
        </div>
    );
};

export default MetadataItem;