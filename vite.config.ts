import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  optimizeDeps: { include: ['@xterm/xterm', '@xterm/addon-fit', '@xterm/addon-search'] },
  server: {
    port: 1420, strictPort: true,
    watch: { ignored: ['**/src-tauri/**', '**/.runtime-work/**', '**/.build-tools/**', '**/artifacts/**', '**/playwright-report/**', '**/test-results/**'] },
  },
  clearScreen: false,
});
