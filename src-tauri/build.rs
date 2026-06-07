//! 构建脚本
//! 
//! 在编译前由 Cargo 自动执行，主要用于 Tauri 的资源准备和代码生成。

/// Tauri 构建入口
/// 
/// 执行 Tauri 框架所需的编译前准备：
/// - 生成 Windows 资源文件（图标等）
/// - 配置 macOS 的 Info.plist
/// - 处理前端静态资源路径
fn main() {
    tauri_build::build()
}
