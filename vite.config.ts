import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Cargo build artifacts: thousands of files, never relevant to HMR.
      ignored: ['**/target/**', '**/dist/**', '**/node_modules/**'],
    },
  },
});
