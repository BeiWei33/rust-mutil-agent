/**
 * React 应用入口 — 渲染根组件
 */

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// 挂载到 #root 元素
const rootEl = document.getElementById("root");
if (!rootEl) {
  throw new Error("找不到 #root 挂载点，请检查 index.html");
}

ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
