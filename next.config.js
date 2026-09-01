/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Static export: the app is served by the Tauri webview from ../out,
  // there is no Node server at runtime.
  output: "export",
  typescript: {
    ignoreBuildErrors: true,
  },
  eslint: {
    ignoreDuringBuilds: true,
  },

  images: {
    // next/image optimization needs a server — disabled for static export
    unoptimized: true,
  },

  webpack: (config, { isServer }) => {
    // Ignore non-source directories to prevent unnecessary Fast Refresh rebuilds
    config.watchOptions = {
      ...config.watchOptions,
      ignored: ['**/.*/**', '**/node_modules/**', '**/src-tauri/**', '**/out/**'],
    };

    // Handle Three.js and other large dependencies
    if (!isServer) {
      config.resolve.fallback = {
        ...config.resolve.fallback,
        fs: false,
        net: false,
        tls: false,
      };
    }

    return config;
  },
};

module.exports = nextConfig;
