import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig(({ command }) => ({
  // Desktop bundles must use relative asset paths.
  base: command === 'serve' ? '/' : './',
  plugins: [react()],
  clearScreen: false,
  server: {
    strictPort: true,
    port: 5173,
  },
}))
