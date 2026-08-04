import { defineConfig } from 'vite'
import preact from '@preact/preset-vite'
import { resolve } from 'path'

export default defineConfig({
  plugins: [preact()],
  base: '/ui/',
  build: {
    outDir: '../../dist/dashboard',
    emptyOutDir: true,
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  server: {
    port: 5174,
    proxy: {
      '/api': 'http://localhost:3456',
      '/ws': { target: 'ws://localhost:3456', ws: true },
      '/health': 'http://localhost:3456',
    },
  },
  css: {
    modules: {
      localsConvention: 'camelCaseOnly' as const,
    },
  },
})
