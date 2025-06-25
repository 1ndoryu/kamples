import React from 'react';

const TagIcon = ({ className }: { className?: string }) => (
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
            d="M11 4H4a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2V11M14 4l6 6m-6-6h.01M14 4l6 6m-6-6h.01"
        />
    </svg>
);

export default TagIcon;