import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react-swc'
import wasm from "vite-plugin-wasm";
// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), wasm()],
  resolve: {
    alias: [
      { find: '@', replacement: "/src" },
    ]
  },
  define: {
    __INJECTED_HOST_NAME__: JSON.stringify(process.env.API_HOST ?? "localhost:3000")
  }
})
