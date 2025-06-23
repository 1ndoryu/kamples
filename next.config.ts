import type {NextConfig} from 'next';

const nextConfig: NextConfig = {
    images: {
        remotePatterns: [
            {
                protocol: 'https',
                hostname: 'picsum.photos',
                port: '',
                pathname: '/**'
            },
            {
                protocol: 'https',
                hostname: 'swordphp.com',
                port: '',
                pathname: '/**'
            }
        ]
    },
    eslint: {
        // Warning: Esto permitirá que la producción se compile con errores de ESLint.
        ignoreDuringBuilds: true
    },
};

export default nextConfig;
