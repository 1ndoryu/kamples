import React from 'react';

const KeyIcon = ({ className }: { className?: string }) => (
    <svg
        className={className}
        xmlns="http://www.w3.org/2000/svg"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
    >
        <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M15.5 14.5l-3-3m0 0l-3 3m3-3V3a3 3 0 10-6 0v3.586a3 3 0 00.879 2.121l3 3m0 0l3 3m-3-3a3 3 0 106 0v-3.586a3 3 0 00-.879-2.121l-3-3z"
        />
    </svg>
);

export default KeyIcon;