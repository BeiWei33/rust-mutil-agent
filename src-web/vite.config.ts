import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],

  // Vite 环境变量前缀（Tauri 建议使用 TAURI_ 以外的前缀）
  envPrefix: ["VITE_", "TAURI_ENV_"],

  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  // 开发服务器配置（Tauri 兼容）
  server: {
    // Tauri 在开发模式下需要严格的端口
    strictPort: true,
    // 允许 Tauri WebView 连接
    host: "localhost",
    port: 1420,
    // HMR 配置
    watch: {
      // 排除 src-tauri 目录避免不必要的重载
      ignored: ["**/src-tauri/**"],
    },
  },

  // 构建配置
  build: {
    // Tauri 使用 Chromium，设置合适的构建目标
    target: "esnext",
    // 输出目录
    outDir: "dist",
    // 生成 source map 便于调试
    sourcemap: !!process.env.TAURI_DEBUG,
  },

  // 清除 Vite 对 Tauri 环境的警告
  clearScreen: false,

  // Vitest 测试配置
  test: {
    // 使用 jsdom 模拟浏览器环境（支持 DOM API 和 localStorage）
    environment: "jsdom",
    // 全局测试 API（describe, it, expect 等无需导入）
    globals: true,
    // 测试文件匹配模式
    include: ["src/**/*.test.{ts,tsx}"],
    // CSS 处理（与组件测试兼容）
    css: true,
    // setup 文件
    setupFiles: ["./src/test/setup.ts"],
  },
});
