# rspin 项目亮点与技术栈

## 🌟 项目亮点 / Project Highlights

1. **原生 Wayland 支持**

- 直接使用 Wayland 协议，无需 X11 兼容层
- 利用 layer-shell 协议实现真正的桌面叠加层
- 完美支持现代 Linux 桌面环境（GNOME、KDE Plasma、Hyprland 等）

2. **高性能 GPU 加速渲染**

   - 使用 wgpu 实现跨平台 GPU 渲染（支持 Vulkan/OpenGL）
   - 智能后备机制：GPU 不可用时自动切换到 CPU 渲染
   - 实时图像缩放和透明度调整，无性能损失
3. **极致内存优化**

   - 启动内存占用 < 100MB（相比初始版本减少 90%）
   - GPU 纹理上传后自动释放 CPU 端图像数据
   - 字体系统延迟加载，仅在需要时初始化
   - 智能资源管理：关闭菜单时释放字体资源
4. **丰富的交互功能**

   - 拖拽边缘调整大小
   - 拖拽中心移动位置
   - 鼠标滚轮调整透明度
   - 右键菜单提供完整功能访问
   - 双击关闭窗口
5. **完善的打包支持**

   - 支持 DEB、RPM、Arch Linux 包格式
   - 提供便携式 tar.gz 包（带安装脚本）
   - Nix Flakes 支持（NixOS/home-manager 集成）
   - GitHub Actions 自动化构建和发布


1. **Native Wayland Support**

   - Direct Wayland protocol usage without X11 compatibility layer
   - True desktop overlay using layer-shell protocol
   - Perfect support for modern Linux desktop environments (GNOME, KDE Plasma, Hyprland, etc.)
2. **High-Performance GPU-Accelerated Rendering**

   - Cross-platform GPU rendering using wgpu (Vulkan/OpenGL support)
   - Intelligent fallback: automatic CPU rendering when GPU unavailable
   - Real-time image scaling and opacity adjustment with zero performance penalty
3. **Extreme Memory Optimization**

   - Startup memory footprint < 100MB (90% reduction from initial version)
   - Automatic CPU-side image data release after GPU texture upload
   - Lazy-loaded font system, initialized only when needed
   - Smart resource management: font resources freed when menu closes
4. **Rich Interactive Features**

   - Drag edges to resize
   - Drag center to move
   - Mouse wheel to adjust opacity
   - Right-click menu for full feature access
   - Double-click to close window
5. **Comprehensive Packaging Support**

   - DEB, RPM, Arch Linux package formats
   - Portable tar.gz with installation scripts
   - Nix Flakes support (NixOS/home-manager integration)
   - Automated build and release via GitHub Actions

---

## 🛠️ 技术栈 / Tech Stack

### 核心技术 / Core Technologies

**Rust 生态系统 / Rust Ecosystem**

- `smithay-client-toolkit` - Wayland 客户端工具包 / Wayland client toolkit
- `wayland-client` - Wayland 协议绑定 / Wayland protocol bindings
- `wayland-protocols` - 扩展协议支持 / Extended protocol support

**图形渲染 / Graphics Rendering**

- `wgpu` - 现代跨平台 GPU API / Modern cross-platform GPU API
- `bytemuck` - 零成本类型转换 / Zero-cost type conversions
- `raw-window-handle` - 窗口系统抽象 / Window system abstraction

**图像处理 / Image Processing**

- `image` - 多格式图像加载和处理 / Multi-format image loading and processing

**字体与文本 / Fonts & Text**

- `cosmic-text` - 高级文本渲染 / Advanced text rendering
- 延迟加载机制 / Lazy-loading mechanism
- 最小化字体集（Noto Sans + Noto Color Emoji）/ Minimized font set

**构建与打包 / Build & Packaging**

- `cargo-deb` - DEB 包生成 / DEB package generation
- `cargo-generate-rpm` - RPM 包生成 / RPM package generation
- `makepkg` - Arch Linux 包构建 / Arch Linux package building
- Nix Flakes - 声明式包管理 / Declarative package management

**CI/CD**

- GitHub Actions - 多平台自动化构建 / Multi-platform automated builds
- 自动发布到 GitHub Releases / Automatic release to GitHub Releases

### 性能优化技术 / Performance Optimization Techniques

1. **内存管理 / Memory Management**

   - 零拷贝纹理上传（分块处理）/ Zero-copy texture upload (chunked processing)
   - 即时资源释放 / Just-in-time resource deallocation
   - 条件编译优化 / Conditional compilation optimizations
2. **渲染优化 / Rendering Optimization**

   - GPU mipmap 硬件生成 / GPU hardware mipmap generation
   - 双缓冲机制 / Double-buffering mechanism
   - Alpha 预乘优化 / Alpha premultiplication optimization
3. **智能后备 / Intelligent Fallback**

   - GPU → CPU 渲染自动切换 / Automatic GPU → CPU rendering fallback
   - 共享内存池管理 / Shared memory pool management

---

## 📊 性能指标 / Performance Metrics

| 指标 / Metric                      | 优化前 / Before          | 优化后 / After               | 改进 / Improvement |
| ---------------------------------- | ------------------------ | ---------------------------- | ------------------ |
| 启动内存 / Startup Memory          | ~1000 MB                 | < 100 MB                     | **-90%**     |
| 字体加载 / Font Loading            | 4888 字体面 / font faces | 2 字体 / fonts               | **-99.96%**  |
| GPU 纹理上传峰值 / GPU Upload Peak | 完整副本 / Full copy     | 分块处理 / Chunked           | **-75%**     |
| Mipmap 内存 / Mipmap Memory        | +33% 原图 / of original  | 0 (GPU 生成 / GPU-generated) | **-100%**    |
