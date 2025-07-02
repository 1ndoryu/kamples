import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  async rewrites() {
    return [
      {
        source: "/api/:path*",
        destination: "http://127.0.0.1:8787/:path*", // proxy al backend local
      },
    ];
  },
};

export default nextConfig;
