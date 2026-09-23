# Zelland & wgpu-in-app 深度研究报告

> 基于实际代码分析，非推测。所有文件路径均相对于各仓库根目录。

---

## 目录

1. [项目概述](#1-项目概述)
2. [核心功能列表](#2-核心功能列表)
3. [关键源文件分析](#3-关键源文件分析)
4. [依赖分析](#4-依赖分析)
5. [torvox 对比](#5-torvox-对比)
6. [可移植代码片段](#6-可移植代码片段)

---

## 1. 项目概述

### 1.1 Zelland (`/tmp/refs2/zelland`)

**README 摘录**: "A native Android + Linux terminal client built on Tauri, wgpu, and SSH"

**核心目标**: 移动优先的 SSH 终端和命令中心。通过 SSH 直接连接到远程主机，使用 wgpu + glyphon 在 Android 上通过 Vulkan 原生渲染终端，可选地通过本地 Rust 守护进程同步协作式 markdown 注释。

**架构**:
```
Android Activity (MainActivity.kt)
├── DrawerLayout
│   ├── FrameLayout (主内容)
│   │   ├── WebView (Svelte 应用 — 欢迎屏幕、模态框)
│   │   └── SurfaceView (wgpu Vulkan 终端表面)
│   └── LinearLayout (左侧边栏 — 会话 + 主机树)
│
├── GestureDetector: tap → focus, 2-finger scroll, pinch zoom
└── KeybarPlugin: IME 工具栏，带 Ctrl / Alt / Meta 修饰键
         │ JNI                              │ JS bridge
         ▼                                  ▼
┌─────────────────┐              ┌──────────────────────┐
│  Rust (src-tauri)│             │  Svelte 5 前端        │
│  wgpu renderer  │              │  welcome / modals     │
│  SSH manager    │              │  markdown pane        │
│  libghostty-vt  │              │  annotation editor    │
└────────┬────────┘              └──────────────────────┘
         │
         ▼
┌──────────────────────────────────────────┐
│  russh → SSH channel → remote host       │
│  VT bytes → libghostty-vt → render state │
│  Touch → SGR mouse sequences → SSH       │
└──────────────────────────────────────────┘
         │ HTTP REST + WebSocket
         ▼
┌──────────────────────────────────────────┐
│  zellandd (daemon-rs)                    │
│  axum + tokio · project/asset/annotation │
│  YJS-based collaborative sync            │
└──────────────────────────────────────────┘
```

**技术栈**:
- **前端**: Svelte 5 + Tauri v2
- **后端**: Rust (src-tauri)
- **终端解析**: libghostty-vt (C FFI 绑定)
- **SSH**: russh 0.57
- **渲染**: wgpu 23.0 + glyphon 0.7
- **JNI**: jni 0.21 + ndk 0.9
- **守护进程**: axum + tokio (daemon-rs/)

---

### 1.2 wgpu-in-app (`/tmp/refs2/wgpu-in-app`)

**README 摘录**: "Integrate wgpu into your existing iOS | Android apps."

**核心目标**: 展示如何将 wgpu 集成到现有非游戏应用中，无需 winit 等第三方窗口管理库。提供跨平台（iOS/Android/Desktop/Web）的 wgpu 表面抽象层。

**架构**:
```
┌─────────────────────────────────────────────────────┐
│  AppSurface (app-surface crate)                      │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌───────────┐  │
│  │  iOS    │ │ Android │ │ Desktop │ │   Web     │  │
│  │ CAMetal │ │ ANative │ │  winit  │ │ web-sys   │  │
│  │ Layer   │ │ Window  │ │         │ │           │  │
│  └────┬────┘ └────┬────┘ └────┬────┘ └─────┬─────┘  │
│       └───────────┴───────────┴────────────┘        │
│                      │                               │
│              wgpu::Instance                          │
│              wgpu::Adapter                           │
│              wgpu::Device                            │
│              wgpu::Queue                             │
│              wgpu::Surface                           │
└─────────────────────────────────────────────────────┘
                      │
                      ▼
              WgpuCanvas (lib.rs)
              ├── Boids 示例
              ├── Cube 示例
              ├── Water 示例
              ├── Shadow 示例
              ├── MSAA Line 示例
              └── HDR Image 示例
```

**技术栈**:
- **核心**: wgpu 30 (更新版本)
- **平台抽象**: app-surface crate (自建)
- **桌面**: winit 0.30
- **移动端**: jni 0.21 + ndk-sys + jni_fn
- **Web**: wasm-bindgen + web-sys
- **日志**: android_logger / env_logger
- **数学**: glam 0.32

---

## 2. 核心功能列表

### 2.1 Zelland 核心功能

#### 终端渲染
- **wgpu + glyphon** 通过 Vulkan (Android) 或系统 GPU (Linux) 渲染
- 完整 **ANSI 颜色**支持: 16色调色板 + 24位 RGB
- **Bold**, *italic*, reverse-video, underline 属性
- 硬件加速光标矩形 (自定义 WGSL shader)
- **文本选择** 带原生 Copy/Paste 操作栏
- **Pinch-to-zoom** 字体大小缩放 (通过 ScaleGestureDetector)
- 行脏矩形缓存 (`row_cache: Vec<Vec<CellRun>>`) 优化重绘

#### SSH & 连接
- 直接 SSH 支持密码或公钥认证 (`russh`)
- **SSH keepalives** (30秒间隔) 在屏幕关闭后台存活
- **WireGuard** 隧道支持 (通过 gotatun crate)
- 每个会话独立的 resize, scroll, SGR 鼠标跟踪

#### Android 原生 UI
- **DrawerLayout 侧边栏**: 滑动打开原生会话 + 主机面板
- **前台服务 + 唤醒锁**: SSH 会话在屏幕锁定时存活
- **KeybarPlugin**: 持久化 IME 工具栏，带锁定 Ctrl/Alt/Meta 修饰键和方向键
- 底部模态框 (Add Host, Add Session, Settings) 通过 WebView 层渲染
- 返回按钮关闭打开的抽屉; 表面生命周期处理屏幕锁定/解锁

#### 协作注释 (`zellandd`)
- 本地 Rust 守护进程服务 projects, assets, 和 markdown 文件
- **YJS-based 实时同步** 用于内联文本和代码块注释
- REST + WebSocket API 被 Svelte markdown 面板消费

#### 密钥管理
- Android KeyStore 集成
- 生物识别认证 (BiometricPrompt)
- 硬件支持的密钥生成
- 主密码加密私钥存储

---

### 2.2 wgpu-in-app 核心功能

#### 跨平台表面创建
- **iOS**: CAMetalLayer 直接创建 Surface
- **Android**: ANativeWindow from Surface (JNI)
- **Desktop**: winit Window + raw-window-handle
- **Web**: HtmlCanvasElement / OffscreenCanvas

#### 触摸事件抽象
- `Touch` 结构体包含 phase, position, stylus_angle, pressure, major_radius
- `TouchPhase` 枚举: Started, Moved, Ended, Cancelled
- 支持 Apple Pencil 方位角和高度角

#### 示例渲染器
- **Boids**: 群体模拟 (compute shader)
- **Cube**: 基础 3D 立方体
- **MSAA Line**: 多重采样抗锯齿线条
- **Water**: 水面渲染 (反射/折射)
- **Shadow**: 阴影映射
- **HDR Image**: HDR 图像显示

#### 工具链
- **cargo-so**: 构建 Android .so 库的子命令
- 支持 `angle` 后端 (OpenGL ES)
- 支持 `vulkan-portability` 后端 (macOS)

---

## 3. 关键源文件分析

### 3.1 Zelland 源文件

#### `src-tauri/src/lib.rs` — Tauri 命令入口

**关键结构**:
```rust
// 行 44: 密钥管理器包装
pub struct ManagedKeyManager(pub Arc<dyn KeyManager>);

// 行 47-64: SSH 连接命令
#[tauri::command]
async fn ssh_connect(
    ssh_state: State<'_, SshManager>,
    key_state: State<'_, ManagedKeyManager>,
    tab_id: String,
    config: SshConfig,
    rows: u16,
    cols: u16,
    channel: Channel<SshChannelMsg>,
) -> Result<(), String>

// 行 67-69: 断开连接
#[tauri::command]
async fn ssh_disconnect(state: State<'_, SshManager>, tab_id: String) -> Result<(), ()>

// 行 73-75: 写入输入
#[tauri::command]
async fn ssh_write(state: State<'_, SshManager>, tab_id: String, data: Vec<u8>) -> Result<(), String>

// 行 78-80: 调整大小
#[tauri::command]
async fn ssh_resize(state: State<'_, SshManager>, tab_id: String, rows: u32, cols: u32) -> Result<(), String>

// 行 129-132: 设置字体大小 (从前端调用)
#[tauri::command]
async fn set_terminal_font_size(css_px: f32, dpr: f32) -> Result<(), String> {
    crate::renderer::update_font_size_global(css_px, dpr);
    Ok(())
}

// 行 184-255: Tauri 运行入口
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SshManager::new())
        .manage(network::NetworkManager::new())
        .manage(DaemonManager::new())
        .plugin(tauri_plugin_log::Builder::new()...)
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_haptics::init())
        .invoke_handler(tauri::generate_handler![...])
        .run(tauri::generate_context!())
}
```

**依赖关系图**:
```
lib.rs
├── ssh.rs (SshManager)
├── terminal.rs (TerminalSession)
├── ghostty.rs (GhosttyTerminalWrapper, GhosttyRenderStateWrapper)
├── renderer/
│   ├── mod.rs (Renderer, wgpu + glyphon)
│   └── android.rs (JNI 入口点)
├── daemon.rs (DaemonManager)
├── keystore.rs (KeyManager trait + AndroidKeyManager)
├── network.rs (WireGuard)
├── helper.rs (远程 helper 管理)
├── intent.rs (Android Intent 插件)
└── keybar.rs (KeybarPlugin Tauri 插件)
```

---

#### `src-tauri/src/ssh.rs` — SSH 会话管理

**核心类型**:
```rust
// 行 19-24: 前端消息枚举
#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum SshChannelMsg { Closed }

// 行 49-54: 认证方式
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AuthMethod { Password, PrivateKey, Key }

// 行 56-68: SSH 配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SshConfig {
    pub host: String, port: u16, username: String,
    pub auth_method: AuthMethod,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
    pub key_id: Option<String>, // KeyStore 管理的密钥 ID
    pub session_name: String,
    pub project_root: Option<String>,
}

// 行 70-74: 会话消息
pub enum SessionMsg {
    Data(Vec<u8>),
    Resize { rows: u32, cols: u32 },
    ProcessMouse { x: f32, y: f32, action: String },
}

// 行 136-141: SSH 管理器
#[derive(Clone)]
pub struct SshManager {
    pub active_sessions: Arc<Mutex<HashMap<String, mpsc::Sender<SessionMsg>>>>,
    pub focused_session: Arc<Mutex<Option<String>>>,
}
```

**关键函数签名**:
```rust
// 行 27-47: 打开认证 SSH 会话
async fn open_session(
    config: &SshConfig,
    key_manager: Arc<dyn KeyManager>,
) -> Result<client::Handle<Client>, String>

// 行 90-111: 加载私钥
async fn load_private_key(
    config: &SshConfig,
    key_manager: Arc<dyn KeyManager>
) -> Result<russh::keys::PrivateKey, String>

// 行 114-133: 认证
async fn authenticate(
    session: &mut client::Handle<Client>,
    config: &SshConfig,
    key_manager: Arc<dyn KeyManager>,
) -> Result<AuthResult, String>

// 行 150-181: 运行一次性命令 (用于 zellij list-sessions 等)
pub async fn run_command(&self, config: SshConfig, cmd: String, key_manager: Arc<dyn KeyManager>) -> Result<String, String>

// 行 183-233: 上传文件
pub async fn upload_file(&self, config: SshConfig, remote_path: &str, contents: &[u8], key_manager: Arc<dyn KeyManager>) -> Result<(), String>

// 行 235-365: 主连接循环
pub async fn connect(&self, tab_id: String, config: SshConfig, rows: u16, cols: u16, output: Channel<SshChannelMsg>, key_manager: Arc<dyn KeyManager>) -> Result<(), String>
```

**关键实现细节**:
- **行 265**: `build_zellij_connect_command` 构建连接命令，支持项目根目录
- **行 277-354**: `tokio::spawn` 循环处理 `SessionMsg` 和 `ChannelMsg`，16ms 刷新间隔 (60 FPS)
- **行 304-308**: Resize 通过 `channel.window_change` 发送到 SSH

---

#### `src-tauri/src/terminal.rs` — 终端会话封装

**核心结构**:
```rust
// 行 4-8: 终端会话
pub struct TerminalSession {
    term: GhosttyTerminalWrapper,
    render_state: GhosttyRenderStateWrapper,
    dirty: bool,
}
```

**关键函数**:
```rust
// 行 11-21: 创建新会话
pub fn new(cols: u16, rows: u16) -> Result<Self, String>

// 行 23-26: 处理输入字节
pub fn process_bytes(&mut self, data: &[u8>) {
    self.term.write(data);
    self.dirty = true;
}

// 行 28-35: 调整大小
pub fn resize(&mut self, cols: u16, rows: u16) {
    let width_px = cols as u32 * crate::renderer::CELL_WIDTH as u32;
    let height_px = rows as u32 * crate::renderer::CELL_HEIGHT as u32;
    if let Err(e) = self.term.resize(cols, rows, width_px, height_px) { ... }
    self.dirty = true;
}

// 行 51-93: 处理鼠标事件 (scroll_up/down, click)
pub fn process_mouse(&self, x: f32, y: f32, action: &str) -> Vec<Vec<u8>>

// 行 96-175: 编码鼠标事件为 ANSI SGR 序列
pub fn encode_mouse_event(&self, x_px: f32, y_px: f32, action: &str) -> Option<Vec<u8>>

// 行 180-200: 渲染帧
pub fn render_native(&mut self) -> Result<(), String> {
    self.render_state.update(&self.term)?;
    let cursor_pos = self.term.get_cursor_pos();
    let had_renderer = crate::renderer::with_renderer(|renderer| {
        renderer.draw_ghostty_state(&mut self.render_state, cursor_pos);
        renderer.render();
    }).is_some();
    if had_renderer {
        self.dirty = false;
        self.render_state.reset_dirty();
    }
    Ok(())
}
```

---

#### `src-tauri/src/renderer/mod.rs` — wgpu + glyphon 渲染器

**常量定义** (行 15-16):
```rust
pub const CELL_WIDTH: f32 = 17.0;
pub const CELL_HEIGHT: f32 = 38.0;
```

**WGSL Shaders**:
```rust
// 行 19-39: 光标矩形 shader
const CURSOR_SHADER: &str = r#"
struct CursorUniforms { rect: vec4<f32>, color: vec4<f32>; }
@vertex fn vs_main(...) -> @builtin(position) vec4<f32> { ... }
@fragment fn fs_main() -> @location(0) vec4<f32> { return u.color; }
"#;

// 行 41-50: 选择高亮 shader
const SELECTION_SHADER: &str = r#"
@vertex fn vs_main(@location(0) pos: vec2<f32>) -> @builtin(position) vec4<f32> { ... }
@fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(0.3, 0.5, 1.0, 0.35); }
"#;
```

**Renderer 结构** (行 62-105):
```rust
pub struct Renderer {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    pending_size: Option<(u32, u32)>,
    // Glyphon 文本渲染
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    atlas_format: wgpu::TextureFormat,
    text_renderer: TextRenderer,
    viewport: Viewport,
    text_buffer: glyphon::Buffer,
    // 行脏缓存
    row_cache: Vec<Vec<CellRun>>,
    span_buf: Vec<(String, Weight, Style, Color)>,
    cell_width: f32, cell_height: f32,
    // 光标
    cursor_pipeline: Option<wgpu::RenderPipeline>,
    cursor_bind_group_layout: Option<wgpu::BindGroupLayout>,
    cursor_uniform_buf: Option<wgpu::Buffer>,
    cursor_bind_group: Option<wgpu::BindGroup>,
    cursor_pixel_rect: Option<(f32, f32, f32, f32)>,
    // 选择
    selection: Option<(i32, i32, i32, i32)>,
    selection_pipeline: Option<wgpu::RenderPipeline>,
    selection_vertex_buf: Option<wgpu::Buffer>,
    selection_vertex_count: u32,
}
```

**关键函数**:
```rust
// 行 130-269: 初始化 wgpu
pub async fn init() { ... }

// 行 271-330: 设置表面 (JNI 调用)
pub fn set_surface(&mut self, window: RawWindow, display: RawDisplay) { ... }

// 行 335-350: 重建文本管线 (格式变化时)
fn rebuild_text_pipeline(&mut self, format: wgpu::TextureFormat) { ... }

// 行 354-428: 构建光标 shader 资源
fn build_cursor_resources(&mut self) { ... }

// 行 430-490: 构建选择高亮资源
fn build_selection_resources(&mut self) { ... }

// 行 492-530: 调整大小
pub fn resize(&mut self, width: u32, height: u32) { ... }

// 行 532-740: 渲染循环
pub fn render(&mut self) { ... }

// 行 842-962: 绘制 Ghostty 状态 (核心渲染逻辑)
pub fn draw_ghostty_state(&mut self, state: &mut GhosttyRenderStateWrapper, cursor_pos: (u16, u16)) { ... }

// 行 966-1028: 构建行 runs
fn build_row_runs(cells: &GhosttyRenderStateRowCells) -> Vec<CellRun> { ... }
```

**CellRun 结构** (行 53-60):
```rust
#[derive(Clone, PartialEq)]
struct CellRun {
    text: String,
    fg: (u8, u8, u8),
    bold: bool,
    italic: bool,
}
```

**ANSI 调色板** (行 1051-1082):
```rust
fn ansi_palette_color(idx: usize) -> (u8, u8, u8) {
    const PALETTE: [(u8, u8, u8); 16] = [
        (0, 0, 0), (170, 0, 0), (0, 170, 0), (170, 170, 0),
        (0, 0, 170), (170, 0, 170), (0, 170, 170), (170, 170, 170),
        (85, 85, 85), (255, 85, 85), (85, 255, 85), (255, 255, 85),
        (85, 85, 255), (255, 85, 255), (85, 255, 255), (255, 255, 255),
    ];
    // 6x6x6 色立方 (16-231) + 灰度渐变 (232-255)
}
```

---

#### `src-tauri/src/renderer/android.rs` — JNI 入口点

**JNI 函数**:
```rust
// 行 18-59: 传递 Surface 到 Rust
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_njr_zelland_MainActivity_passSurfaceToRust(
    env: JNIEnv, _class: JClass, surface: JObject) { ... }

// 行 62-74: Surface 销毁通知
pub extern "system" fn Java_com_njr_zelland_MainActivity_passSurfaceDestroyedToRust(...) { ... }

// 行 77-99: 调整大小
pub extern "system" fn Java_com_njr_zelland_MainActivity_passResizeToRust(
    _env: JNIEnv, _class: JClass, width: jint, height: jint) { ... }

// 行 102-118: 获取选择文本
pub extern "system" fn Java_com_njr_zelland_MainActivity_getSelectionText(...) -> jstring { ... }

// 行 121-138: 设置选择高亮
pub extern "system" fn Java_com_njr_zelland_MainActivity_setSelectionHighlight(...) { ... }

// 行 141-161: 粘贴
pub extern "system" fn Java_com_njr_zelland_MainActivity_passPasteToRust(
    mut env: JNIEnv, _class: JClass, data: JByteArray) { ... }

// 行 164-173: 获取单元格尺寸
pub extern "system" fn Java_com_njr_zelland_MainActivity_getCellDimensions(...) -> jfloatArray { ... }

// 行 176-191: 更新字体大小
pub extern "system" fn Java_com_njr_zelland_MainActivity_updateFontSizeToRust(
    _env: JNIEnv, _class: JClass, physical_px: jfloat) { ... }

// 行 194-216: 触摸事件
pub extern "system" fn Java_com_njr_zelland_MainActivity_passTouchToRust(
    mut env: JNIEnv, _class: JClass, action: JString, x: jfloat, y: jfloat) { ... }
```

---

#### `src-tauri/src/ghostty.rs` — C FFI 绑定

**GhosttyTerminalWrapper** (行 15-117):
```rust
pub struct GhosttyTerminalWrapper {
    pub(crate) inner: GhosttyTerminal,
}

impl GhosttyTerminalWrapper {
    pub fn new(cols: u16, rows: u16) -> Result<Self, i32> {
        let options = GhosttyTerminalOptions { cols, rows, max_scrollback: 0 };
        let result = unsafe { ghostty_terminal_new(ptr::null(), &mut terminal, options) };
        ...
    }
    pub fn write(&self, data: &[u8]) {
        unsafe { ghostty_terminal_vt_write(self.inner, data.as_ptr(), data.len()); }
    }
    pub fn resize(&self, cols: u16, rows: u16, width_px: u32, height_px: u32) -> Result<(), i32> { ... }
    pub fn get_size(&self) -> (u16, u16) { ... }
    pub fn get_cursor_pos(&self) -> (u16, u16) { ... }
    pub fn get_mouse_tracking(&self) -> bool { ... }
}
```

**GhosttyRenderStateWrapper** (行 121-284):
```rust
pub struct GhosttyRenderStateWrapper {
    inner: GhosttyRenderState,
    row_iter: GhosttyRenderStateRowIterator,
    row_cells: GhosttyRenderStateRowCells,
}

impl GhosttyRenderStateWrapper {
    pub fn new() -> Result<Self, i32> { ... }
    pub fn update(&self, terminal: &GhosttyTerminalWrapper) -> Result<(), i32> { ... }
    pub fn get_dirty(&self) -> GhosttyRenderStateDirty { ... }
    pub fn reset_dirty(&self) { ... }
    pub fn get_size(&self) -> (u16, u16) { ... }
    pub fn with_rows<F>(&mut self, mut f: F) where F: FnMut(u16, bool, &GhosttyRenderStateRowCells) { ... }
}
```

**辅助函数** (行 289-335):
```rust
pub fn get_cell_raw(cells: GhosttyRenderStateRowCells) -> GhosttyCell { ... }
pub fn get_cell_style(cells: GhosttyRenderStateRowCells) -> GhosttyStyle { ... }
pub fn get_cell_graphemes(cells: GhosttyRenderStateRowCells) -> Vec<u32> { ... }
```

---

#### `src-tauri/src/keystore.rs` — 密钥管理

**KeyManager trait** (行 18-36):
```rust
#[async_trait]
pub trait KeyManager: Send + Sync {
    async fn generate_key(&self, label: String) -> Result<KeyIdentity, String>;
    async fn list_identities(&self) -> Result<Vec<KeyIdentity>, String>;
    async fn delete_identity(&self, id: String) -> Result<(), String>;
    async fn sign(&self, id: String, data: &[u8], reason: String) -> Result<Vec<u8>, String>;
    async fn get_russh_key(&self, id: &str) -> Result<russh::keys::PrivateKey, String>;
}
```

**StandardKeyManager** (行 38-179):
```rust
pub struct StandardKeyManager {
    base_path: std::path::PathBuf,
}

impl StandardKeyManager {
    fn master_passphrase(&self) -> Result<Zeroizing<String>, String> { ... } // 行 55-71
    fn load_decrypted_key(&self, id: &str) -> Result<ssh_key::PrivateKey, String> { ... } // 行 74-90
}

impl KeyManager for StandardKeyManager {
    async fn generate_key(&self, label: String) -> Result<KeyIdentity, String> {
        // 行 95-128: 生成 Ed25519 密钥，使用主密码加密存储
    }
    async fn sign(&self, id: String, data: &[u8], _reason: String) -> Result<Vec<u8>, String> {
        // 行 154-167: 使用 ed25519-dalek 签名
    }
}
```

---

#### `src-tauri/gen/android/app/src/main/java/com/njr/zelland/MainActivity.kt` — Android 主活动

**关键结构** (行 49-76):
```kotlin
class MainActivity : TauriActivity() {
    private lateinit var keyStoreManager: KeyStoreManager
    private lateinit var biometricManager: BiometricManager
    private var surfaceView: SurfaceView? = null
    private var webViewRef: WebView? = null
    internal var hiddenEditText: EditText? = null
    internal var keybarPlugin: KeybarPlugin? = null
    private lateinit var mDetector: GestureDetectorCompat
    // 选择状态
    private var selectionActive = false
    private var selStartCol = 0; private var selStartRow = 0
    private var selEndCol = 0;   private var selEndRow = 0
    private var actionMode: ActionMode? = null
    // 捏合缩放
    private lateinit var scaleDetector: ScaleGestureDetector
    private var baseFontSize = 38f
    private var isPinching = false
    // 原生侧边栏
    private var drawerLayout: DrawerLayout? = null
    private var sidebarSessionsList: LinearLayout? = null
    private var sidebarTrashMode = false
    private val expandedHostIds = mutableSetOf<String>()
}
```

**手势处理** (行 105-170):
```kotlin
mDetector = GestureDetectorCompat(this, object : GestureDetector.SimpleOnGestureListener() {
    override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
        passTouchToRust("click", e.x, e.y)
        // 聚焦隐藏 EditText 以附加系统键盘
        hiddenEditText?.let { et -> et.requestFocus(); imm.showSoftInput(et, 0) }
        return true
    }
    override fun onLongPress(e: MotionEvent) {
        // 开始选择
        val (col, row) = pixelToCell(e.x, e.y)
        selStartCol = col; selStartRow = row
        selEndCol = (col + 12).coerceAtMost(255); selEndRow = row
        selectionActive = true
        setSelectionHighlight(selStartCol, selStartRow, selEndCol, selEndRow, true)
        startSelectionActionMode()
    }
    override fun onFling(e1: MotionEvent?, e2: MotionEvent, velocityX: Float, velocityY: Float): Boolean {
        // 左滑打开侧边栏
        if (velocityX < -600f && Math.abs(velocityX) > Math.abs(velocityY) * 1.5f) {
            drawerLayout?.openDrawer(Gravity.START)
            return true
        }
        return false
    }
    override fun onScroll(e1: MotionEvent?, e2: MotionEvent, distanceX: Float, distanceY: Float): Boolean {
        // 双指滚动
        if (e2.pointerCount == 2) {
            val action = if (distanceY > 0) "scroll_down" else "scroll_up"
            passTouchToRust(action, e2.x, e2.y)
            return true
        }
        return false
    }
})

scaleDetector = ScaleGestureDetector(this, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
    override fun onScale(detector: ScaleGestureDetector): Boolean {
        baseFontSize = (baseFontSize * detector.scaleFactor).coerceIn(20f, 80f)
        updateFontSizeToRust(baseFontSize)
        return true
    }
})
```

**选择操作** (行 859-906):
```kotlin
private fun pixelToCell(x: Float, y: Float): Pair<Int, Int> {
    val dims = getCellDimensions()
    val cw = if (dims[0] > 0) dims[0] else 17f
    val ch = if (dims[1] > 0) dims[1] else 38f
    return Pair((x / cw).toInt().coerceAtLeast(0), (y / ch).toInt().coerceAtLeast(0))
}

private fun startSelectionActionMode() {
    actionMode = (surfaceView ?: window.decorView).startActionMode(object : ActionMode.Callback {
        override fun onCreateActionMode(mode: ActionMode, menu: Menu): Boolean {
            menu.add(0, 1, 0, android.R.string.copy).setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            menu.add(0, 2, 1, android.R.string.paste).setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            return true
        }
        override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
            return when (item.itemId) {
                1 -> { doCopy(); mode.finish(); true }
                2 -> { doPaste(); mode.finish(); true }
                else -> false
            }
        }
        override fun onDestroyActionMode(mode: ActionMode) {
            selectionActive = false
            setSelectionHighlight(0, 0, 0, 0, false)
            actionMode = null
        }
    }, ActionMode.TYPE_FLOATING)
}

private fun doPaste() {
    val cm = getSystemService(CLIPBOARD_SERVICE) as ClipboardManager
    val clip = cm.primaryClip ?: return
    if (clip.itemCount == 0) return
    val text = clip.getItemAt(0).coerceToText(this).toString()
    if (text.isEmpty()) return
    val bracketed = "\u001b[200~$text\u001b[201~"  // 括号粘贴模式
    passPasteToRust(bracketed.toByteArray(Charsets.UTF_8))
}
```

**JNI 声明** (行 954-962):
```kotlin
private external fun passSurfaceToRust(surface: Surface)
private external fun passResizeToRust(width: Int, height: Int)
private external fun passTouchToRust(action: String, x: Float, y: Float)
private external fun passSurfaceDestroyedToRust()
private external fun getSelectionText(sc: Int, sr: Int, ec: Int, er: Int): String
private external fun setSelectionHighlight(sc: Int, sr: Int, ec: Int, er: Int, active: Boolean)
private external fun passPasteToRust(data: ByteArray)
private external fun getCellDimensions(): FloatArray
private external fun updateFontSizeToRust(physicalPx: Float)
```

---

#### `src-tauri/gen/android/app/src/main/java/com/njr/zelland/KeybarPlugin.kt` — IME 工具栏

**核心结构** (行 26-48):
```kotlin
class KeybarPlugin(private val activity: Activity, private val webView: WebView) {
    private var keybarView: View? = null
    internal var modCtrl = false
    internal var modAlt  = false
    internal var modMeta = false
    // 锁定状态: 双击锁定修饰键
    private var modCtrlLocked = false
    private var modAltLocked  = false
    private var modMetaLocked = false
    private var lastCtrlTap = 0L
    private val doubleTapMs = 350L
    // 测试接口
    var emit: (name: String, jsonPayload: String) -> Unit = { name, payload ->
        val js = "window.dispatchEvent(new CustomEvent(${name.toJsString()}, {detail:$payload}));"
        activity.runOnUiThread { webView.evaluateJavascript(js, null) }
    }
}
```

**设置** (行 51-111):
```kotlin
fun setup() {
    val contentFrame = activity.window.decorView.findViewById<FrameLayout>(android.R.id.content)
    val inflater = LayoutInflater.from(activity)
    val bar = inflater.inflate(R.layout.native_keybar, contentFrame, false)
    bar.tag = "keybar_root"
    keybarView = bar
    // 将 WRY 根视图 + keybar 包装在垂直 LinearLayout 中
    val wryRoot = contentFrame.getChildAt(0)
    if (wryRoot != null) {
        contentFrame.removeView(wryRoot)
        val wrapper = LinearLayout(activity).apply { orientation = LinearLayout.VERTICAL }
        // 调整 wrapper padding 以保持在系统键盘上方
        ViewCompat.setOnApplyWindowInsetsListener(wrapper) { v, insets ->
            val imeInsets = insets.getInsets(WindowInsetsCompat.Type.ime())
            val systemInsets = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            v.setPadding(0, 0, 0, java.lang.Math.max(imeInsets.bottom, systemInsets.bottom))
            insets
        }
        wrapper.addView(wryRoot, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))
        wrapper.addView(bar, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))
        contentFrame.addView(wrapper, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
    }
    webView.addJavascriptInterface(KeybarBridge(), "KeybarNative")
    setupButtons(bar)
}
```

**修饰键处理** (行 187-221):
```kotlin
private fun handleModifierTap(mod: String) {
    val now = System.currentTimeMillis()
    when (mod) {
        "ctrl" -> {
            if (modCtrlLocked) { modCtrl = false; modCtrlLocked = false }
            else if (modCtrl && now - lastCtrlTap < doubleTapMs) { modCtrlLocked = true }
            else { modCtrl = !modCtrl }
            lastCtrlTap = now
        }
        // alt, meta 类似...
    }
}
```

---

#### `daemon-rs/src/server.rs` — 守护进程服务器

**AppState** (行 21-28):
```rust
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub asset_manager: AssetManager,
    pub registry: ClientRegistry,
    pub watcher_tx: mpsc::Sender<WatchCommand>,
    pub loro_manager: Arc<LoroManager>,
}
```

**路由** (行 30-71):
```rust
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/projects", get(handlers::projects::list_projects))
        .route("/api/v1/projects/activate", post(handlers::projects::activate_project))
        .route("/api/v1/projects/{id}/files", get(handlers::projects::list_project_files))
        .route("/api/v1/meta/version", get(handlers::meta::get_version))
        .route("/api/v1/fs/read", get(handlers::fs::read_file))
        .route("/api/v1/fs/annotate", post(handlers::fs::annotate_file))
        .route("/api/v1/sessions/recent", get(handlers::sessions::get_recent_sessions).post(handlers::sessions::record_session))
        .nest("/api/v1/trigger", trigger_routes)
        .route("/assets/{id}", get(handlers::assets::serve_asset))
        .route("/ws", get(ws_upgrade))
        .route("/annotations/sync/{*filepath}", get(handlers::annotations::annotation_sync))
        .route("/annotations/{*filepath}", get(handlers::annotations::get_annotations).put(handlers::annotations::put_annotations))
        .with_state(state)
}
```

---

### 3.2 wgpu-in-app 源文件

#### `wgpu-in-app/src/lib.rs` — 库入口

```rust
mod examples;
mod wgpu_canvas;
pub use wgpu_canvas::WgpuCanvas;

#[cfg_attr(target_os = "ios", path = "ffi/ios.rs")]
#[cfg_attr(target_os = "android", path = "ffi/android.rs", allow(non_snake_case))]
mod ffi;

fn init_logger() {
    cfg_select! {
        target_os = "android" => {
            android_logger::init_once(
                android_logger::Config::default().with_max_level(log::LevelFilter::Info)
            );
            log_panics::init();
        }
        _ => {
            env_logger::builder()
                .filter_level(log::LevelFilter::Info)
                .filter_module("wgpu_core", log::LevelFilter::Info)
                .filter_module("wgpu_hal", log::LevelFilter::Error)
                .filter_module("naga", log::LevelFilter::Error)
                .parse_default_env()
                .init();
        }
    }
}
```

---

#### `wgpu-in-app/src/wgpu_canvas.rs` — 画布封装

```rust
pub struct WgpuCanvas {
    pub app_surface: AppSurface,
    example: Box<dyn Example>,
}

impl WgpuCanvas {
    pub fn new(app_surface: AppSurface, idx: i32) -> Self { ... }
    pub fn enter_frame(&mut self) { ... }
    pub fn resize(&mut self) { ... }
    pub fn change_example(&mut self, index: i32) { ... }
    fn create_a_example(app_surface: &mut AppSurface, index: i32) -> Box<dyn Example> { ... }
}
```

---

#### `wgpu-in-app/src/ffi/android.rs` — Android JNI 桥

```rust
#[unsafe(no_mangle)]
#[jni_fn("name.jinleili.wgpu.RustBridge")]
pub fn createWgpuCanvas(env: *mut JNIEnv, _: JClass, surface: jobject, idx: jint) -> jlong {
    crate::init_logger();
    let canvas = WgpuCanvas::new(AppSurface::new(env as *mut _, surface), idx);
    Box::into_raw(Box::new(canvas)) as jlong
}

#[unsafe(no_mangle)]
#[jni_fn("name.jinleili.wgpu.RustBridge")]
pub fn enterFrame(_env: *mut JNIEnv, _: JClass, obj: jlong) {
    let obj = unsafe { &mut *(obj as *mut WgpuCanvas) };
    obj.enter_frame();
}

#[unsafe(no_mangle)]
#[jni_fn("name.jinleili.wgpu.RustBridge")]
pub fn dropWgpuCanvas(_env: *mut JNIEnv, _: JClass, obj: jlong) {
    let _obj: Box<WgpuCanvas> = unsafe { Box::from_raw(obj as *mut _) };
}
```

---

#### `app-surface/src/lib.rs` — 平台抽象层

**IASDQContext** (行 152-174):
```rust
#[derive(Clone)]
pub struct IASDQContext {
    pub instance: wgpu::Instance,
    pub surface: SharedPtr<wgpu::Surface<'static>>,
    pub config: wgpu::SurfaceConfiguration,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl IASDQContext {
    pub fn update_config_format(&mut self, format: wgpu::TextureFormat) { ... }
}
```

**SurfaceFrame trait** (行 198-250):
```rust
pub trait SurfaceFrame {
    fn view_size(&self) -> ViewSize;
    fn resize_surface(&mut self);
    fn resize_surface_by_size(&mut self, size: (u32, u32));
    fn pintch(&mut self, _touch: Touch, _scale: f32) {}
    fn touch(&mut self, _touch: Touch) {}
    fn normalize_touch_point(&self, _touch_point_x: f32, _touch_point_y: f32) -> (f32, f32) { unimplemented!() }
    fn enter_frame(&mut self) {}
    fn get_current_frame_view(&self, _view_format: Option<wgpu::TextureFormat>) -> Option<(wgpu::SurfaceTexture, wgpu::TextureView)> { unimplemented!() }
    fn create_current_frame_view(&self, device: &wgpu::Device, surface: &wgpu::Surface, config: &wgpu::SurfaceConfiguration, view_format: Option<wgpu::TextureFormat>) -> Option<(wgpu::SurfaceTexture, wgpu::TextureView)> { ... }
}
```

**create_iasdq_context** (行 306-363):
```rust
async fn create_iasdq_context(
    instance: Instance,
    surface: Surface<'static>,
    physical_size: (u32, u32),
) -> IASDQContext {
    let physical_size = normalize_view_size(physical_size);
    let (adapter, device, queue) = crate::request_device(&instance, &surface).await;
    let caps = surface.get_capabilities(&adapter);
    let prefered = caps.formats[0];
    let format = if cfg!(all(target_arch = "wasm32", not(feature = "webgl"))) {
        prefered.remove_srgb_suffix()
    } else { prefered };
    let view_formats = if cfg!(feature = "webgl") { vec![] }
        else if cfg!(target_os = "android") { vec![format] }  // Android 不支持 view_formats
        else if format.is_srgb() { vec![format, format.remove_srgb_suffix()] }
        else { vec![format.add_srgb_suffix(), format.remove_srgb_suffix()] };
    let mut config = surface.get_default_config(&adapter, physical_size.0, physical_size.1)
        .expect("Surface isn't supported by the adapter.");
    config.view_formats = view_formats;
    config.format = format;
    surface.configure(&device, &config);
    IASDQContext { instance, surface: SharedPtr::new(surface), config, adapter, device, queue }
}
```

---

#### `app-surface/src/android.rs` — Android 表面实现

**AppSurface** (行 10-48):
```rust
pub struct AppSurface {
    pub native_window: Arc<NativeWindow>,
    pub scale_factor: f32,
    pub ctx: crate::IASDQContext,
    pub callback_to_app: Option<extern "C" fn(arg: i32)>,
}

impl AppSurface {
    pub fn new(env: *mut JNIEnv, surface: jobject) -> Self {
        let native_window = Arc::new(NativeWindow::new(env, surface));
        let backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(native_window.clone()))
        });
        let handle: Box<dyn wgpu::WindowHandle> = Box::new(native_window.clone());
        let surface = instance.create_surface(wgpu::SurfaceTarget::Window(handle)).unwrap();
        let ctx = futures_lite::future::block_on(crate::create_iasdq_context(
            instance, surface, native_window.view_size(),
        ));
        Self { native_window, scale_factor: crate::normalize_scale_factor(1.0), ctx, callback_to_app: None }
    }
}
```

**NativeWindow** (行 50-120):
```rust
pub struct NativeWindow {
    a_native_window: Arc<Mutex<*mut ndk_sys::ANativeWindow>>,
}

impl NativeWindow {
    fn new(env: *mut JNIEnv, surface: jobject) -> Self {
        let a_native_window = unsafe {
            ndk_sys::ANativeWindow_fromSurface(env as *mut _, surface as *mut _)
        };
        Self { a_native_window: Arc::new(Mutex::new(a_native_window)) }
    }
    fn get_width(&self) -> u32 { unsafe { ndk_sys::ANativeWindow_getWidth(*self.a_native_window.lock().unwrap()) as u32 } }
    fn get_height(&self) -> u32 { unsafe { ndk_sys::ANativeWindow_getHeight(*self.a_native_window.lock().unwrap()) as u32 } }
    fn view_size(&self) -> (u32, u32) { crate::normalize_view_size((self.get_width(), self.get_height())) }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe { ndk_sys::ANativeWindow_release(*self.a_native_window.lock().unwrap()); }
    }
}

impl HasWindowHandle for NativeWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        unsafe {
            let a_native_window = self.a_native_window.lock().unwrap();
            let handle = AndroidNdkWindowHandle::new(
                core::ptr::NonNull::new(*a_native_window as *mut _ as *mut c_void).unwrap(),
            );
            Ok(WindowHandle::borrow_raw(RawWindowHandle::AndroidNdk(handle)))
        }
    }
}
```

---

#### `app-surface/src/touch.rs` — 触摸事件抽象

```rust
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct StylusAngle<T> {
    pub azimuth: T,
    pub altitude: T,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum TouchPhase { Started, Moved, Ended, Cancelled }

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Touch {
    pub phase: TouchPhase,
    pub position: glam::Vec2,
    pub stylus_angle: Option<StylusAngle<f32>>,
    pub pressure: f32,
    pub major_radius: f32,
    pub interval: f32,
}

impl Touch {
    pub fn touch_start(position: glam::Vec2) -> Self { ... }
    pub fn touch_move(position: glam::Vec2) -> Self { ... }
    pub fn touch_end(position: glam::Vec2) -> Self { ... }
}
```

---

## 4. 依赖分析

### 4.1 Zelland 依赖 (`src-tauri/Cargo.toml`)

| 依赖 | 版本 | 用途 | torvox 适用性评估 |
|------|------|------|------------------|
| **tauri** | 2 | 应用框架 | ❌ torvox 不使用 Tauri，直接 JNI |
| **russh** | 0.57.0 | SSH 客户端 | ❌ torvox 是本地终端，无 SSH |
| **tokio** | 1.49.0 | 异步运行时 | ✅ torvox 使用 tokio (可选，用于 MCP) |
| **wgpu** | 23.0 | GPU 渲染 | ⚠️ torvox 使用 wgpu 30 (更新) |
| **glyphon** | 0.7 | 文本渲染 | ❌ torvox 使用 cosmic-text + swash |
| **raw-window-handle** | 0.6 | 窗口句柄 | ✅ torvox 使用 0.6 |
| **jni** | 0.21 | JNI 绑定 | ✅ torvox 使用 jni (workspace) |
| **ndk** | 0.9.0 | Android NDK | ⚠️ torvox 通过 jni crate 间接使用 |
| **ndk-sys** | 0.6.0 | NDK 系统绑定 | ⚠️ 类似 |
| **prost** | 0.14.3 | Protobuf | ❌ torvox 不使用 protobuf |
| **russh-keys** | 0.49.2 | SSH 密钥 | ❌ torvox 无 SSH |
| **gotatun** | 0.2.0 | WireGuard | ❌ torvox 无网络隧道 |
| **reqwest** | 0.12 | HTTP 客户端 | ⚠️ torvox 可能用于 MCP HTTP |
| **ssh-key** | 0.6 | SSH 密钥管理 | ❌ torvox 无 SSH |
| **ed25519-dalek** | 2 | Ed25519 签名 | ❌ torvox 无 SSH |
| **zeroize** | 1 | 安全清零 | ⚠️ 可能用于敏感数据 |
| **once_cell** | 1 | 懒初始化 | ✅ torvox 使用 std::sync::OnceLock |
| **bytemuck** | 1 | 类型转换 | ✅ torvuck 使用 bytemuck (workspace) |
| **log** | 0.4 | 日志 | ✅ torvox 使用 log (workspace) |
| **uuid** | 1.11 | UUID 生成 | ⚠️ 可能用于会话 ID |
| **chrono** | 0.4 | 时间处理 | ⚠️ 可能用于时间戳 |
| **serde_bytes** | 0.11 | 字节序列化 | ⚠️ 可能用于二进制数据 |
| **async-trait** | 0.1 | async trait | ⚠️ 可能用于 trait 定义 |
| **anyhow** | 1.0 | 错误处理 | ⚠️ torvox 使用 thiserror |
| **bytes** | 1.9 | 字节缓冲 | ⚠️ 可能用于 I/O |
| **base64** | 0.22 | Base64 编码 | ⚠️ 可能用于数据编码 |
| **ipnetwork** | 0.21 | IP 网络 | ❌ torvox 无网络功能 |
| **webkit2gtk** | 2.0.1 | Linux WebView | ❌ torvox 无 WebView |
| **gtk** | 0.18.1 | GTK | ❌ torvox 无 GTK |
| **openssl** | 0.10 | OpenSSL | ❌ torvox 不使用 OpenSSL |

**关键差异**:
- Zelland 依赖 **Tauri** 框架，torvox 直接通过 JNI 与 Kotlin 交互
- Zelland 使用 **russh** 进行 SSH，torvox 是本地 PTY 终端
- Zelland 使用 **glyphon** 进行文本渲染，torvox 使用 **cosmic-text + swash + guillotiere**
- Zelland 使用 **wgpu 23**，torvox 使用 **wgpu 30**
- Zelland 有 **WireGuard** 支持 (gotatun)，torvox 无网络隧道功能

---

### 4.2 wgpu-in-app 依赖 (`Cargo.toml` workspace)

| 依赖 | 版本 | 用途 | torvox 适用性评估 |
|------|------|------|------------------|
| **wgpu** | 30 | GPU 渲染 | ✅ torvox 使用 wgpu 30 |
| **raw-window-handle** | 0.6 | 窗口句柄 | ✅ torvox 使用 0.6 |
| **glam** | 0.32 | 数学库 | ⚠️ torvox 可能用于 2D 数学 |
| **futures-lite** | 2 | 异步工具 | ⚠️ 可能用于异步初始化 |
| **jni** | 0.21 | JNI 绑定 | ✅ torvox 使用 jni |
| **jni_fn** | 0.1 | JNI 宏 | ⚠️ torvox 使用手动 JNI 导出 |
| **ndk-sys** | 0.6 | NDK 系统绑定 | ⚠️ 类似 |
| **android_logger** | 0.15 | Android 日志 | ⚠️ torvox 有自定义 Android 日志 |
| **log** | 0.4 | 日志 | ✅ torvox 使用 log |
| **bytemuck** | 1.22 | 类型转换 | ✅ torvox 使用 bytemuck |
| **winit** | 0.30 | 窗口管理 | ❌ torvox 不使用 winit |
| **objc2** | 0.6 | Objective-C | ❌ torvox 仅 Android |
| **libc** | 0.2 | C 库 | ✅ torvox 使用 libc (workspace) |
| **noise** | 0.9 | 噪声生成 | ❌ 仅示例使用 |
| **rand** | 0.8 | 随机数 | ⚠️ 可能用于测试 |
| **env_logger** | 0.11 | 环境日志 | ⚠️ 桌面端使用 |
| **log-panics** | * | 恐慌日志 | ⚠️ 可能用于调试 |

**关键差异**:
- wgpu-in-app 是**纯 Rust**库，无前端框架
- wgpu-in-app 使用 **winit** 进行桌面窗口管理，torvox 不使用
- wgpu-in-app 支持 **iOS/Web**，torvox 仅 Android
- wgpu-in-app 的 **app-surface** crate 提供跨平台表面抽象，torvox 有自定义表面管理

---

## 5. torvox 对比

### 5.1 torvox 已有功能对比

#### 终端渲染

| 功能 | Zelland | wgpu-in-app | torvox | 实现差异 |
|------|---------|-------------|--------|----------|
| **GPU 渲染** | wgpu 23 + glyphon | wgpu 30 (示例) | wgpu 30 + cosmic-text + swash | torvox 使用更新的 wgpu 版本，自定义字形管线 |
| **文本渲染** | glyphon (TextAtlas) | 无 | cosmic-text + swash + guillotiere | torvox 使用更成熟的文本 shaping |
| **ANSI 颜色** | 16色 + 24位 RGB | 无 | 16色 + 24位 RGB | 类似 |
| **粗体/斜体** | ✅ | 无 | ✅ | torvox 通过 cosmic-text 支持 |
| **光标** | 自定义 WGSL shader | 无 | 自定义 shader | torvox 有更多光标样式 |
| **选择** | 原生 ActionMode | 无 | JNI 回调 | torvox 通过事件队列通知 Kotlin |
| **捏合缩放** | ScaleGestureDetector | 无 | 待确认 | torvox 可能有类似实现 |
| **字体加载** | 文件系统 + 系统字体 | 无 | fontdb + CJK 支持 | torvox 有更完善的字体发现 |
| **Nerd Font** | ✅ (Noto Sans Mono Nerd) | 无 | 待确认 | torvox 可能支持 |

#### 终端引擎

| 功能 | Zelland | wgpu-in-app | torvox | 实现差异 |
|------|---------|-------------|--------|----------|
| **VT 解析** | libghostty-vt (C FFI) | 无 | libghostty-vt (Rust crate) | torvox 使用更深入的 Rust 集成 |
| **PTY** | russh (SSH) | 无 | 本地 PTY (nix crate) | torvox 是本地终端 |
| **会话管理** | SSH 会话 | 无 | 本地会话 (Session) | torvox 有完整的会话生命周期 |
| **滚动缓冲区** | 零滚动 (无本地缓冲) | 无 | 50000 行默认 | torvox 有大型滚动缓冲区 |
| **鼠标跟踪** | SGR 序列 | 无 | SGR 序列 | 类似 |
| **调整大小** | SSH window_change | 无 | PTY winsize + grid | torvox 有本地 resize |

#### Android 集成

| 功能 | Zelland | wgpu-in-app | torvox | 实现差异 |
|------|---------|-------------|--------|----------|
| **JNI 桥** | 手动 JNI 导出 | jni_fn 宏 | 手动 JNI 导出 | torvox 和 zelland 类似 |
| **Surface 生命周期** | SurfaceView + JNI | ANativeWindow | SurfaceView + JNI | 类似 |
| **触摸事件** | GestureDetector | Touch 结构体 | 待确认 | torvox 可能有自定义手势 |
| **键盘输入** | KeybarPlugin + IME | 无 | 待确认 | torvox 可能有自定义键盘 |
| **剪贴板** | 系统剪贴板 | 无 | 系统剪贴板 (OSC 52) | torvox 支持 OSC 52 |
| **前台服务** | TerminalSessionService | 无 | 待确认 | torvox 可能有类似服务 |
| **WebView** | Svelte 前端 | 无 | Jetpack Compose | torvox 使用原生 UI |

#### 密钥管理

| 功能 | Zelland | wgpu-in-app | torvox | 实现差异 |
|------|---------|-------------|--------|----------|
| **密钥存储** | Android KeyStore | 无 | 无 | torvox 无 SSH，不需要 |
| **生物识别** | BiometricPrompt | 无 | 无 | torvox 无此需求 |
| **密钥生成** | Ed25519 | 无 | 无 | torvox 无此需求 |

---

### 5.2 torvox 没有的功能 (来自 Zelland)

#### SSH 客户端功能
- **远程 SSH 连接** (russh)
- **SSH keepalives** (30秒间隔)
- **公钥/密码认证**
- **SSH 密钥管理** (KeyStore 集成)
- **远程命令执行** (run_command)
- **文件上传** (upload_file)
- **Zellij 会话管理** (build_zellij_connect_command)

**实现方案参考** (Zelland `src-tauri/src/ssh.rs`):
```行 235-365```: `SshManager::connect` 建立 SSH 会话并启动 PTY。

#### WireGuard 隧道
- **gotatun** crate 集成
- 配置解析和隧道启动

**实现方案参考** (Zelland `src-tauri/src/network.rs`):
```行 40-80```: `start_tunnel` 命令启动 WireGuard 隧道。

#### 协作注释守护进程
- **daemon-rs** 独立进程
- axum REST + WebSocket 服务器
- YJS 实时同步
- 项目/资产管理

**实现方案参考** (Zelland `daemon-rs/src/server.rs`):
```行 30-71```: `build_router` 定义所有 API 路由。

#### 远程 Helper 管理
- 自动上传和启动远程 helper 二进制文件
- 版本检测和更新

**实现方案参考** (Zelland `src-tauri/src/helper.rs`):
```行 54-119```: `ensure_remote_helper_inner` 确保远程 helper 运行。

#### 生物识别认证
- Android BiometricPrompt 集成
- 密钥解锁

**实现方案参考** (Zelland `MainActivity.kt`):
```行 989-1036```: `authenticate` 和 `authenticateAndDecrypt` 方法。

---

### 5.3 torvox 没有的功能 (来自 wgpu-in-app)

#### 跨平台表面抽象
- **app-surface** crate 提供统一接口
- iOS/Android/Desktop/Web 支持

**实现方案参考** (wgpu-in-app `app-surface/src/lib.rs`):
```行 152-174```: `IASDQContext` 封装 wgpu 资源。

#### 触摸事件抽象
- **Touch** 结构体支持 stylus, pressure, radius

**实现方案参考** (wgpu-in-app `app-surface/src/touch.rs`):
```行 1-53```: `Touch` 和 `TouchPhase` 定义。

#### cargo-so 工具
- 构建 Android .so 库的子命令

**实现方案参考** (wgpu-in-app `cargo-so/src/main.rs`):
构建脚本。

---

## 6. 可移植代码片段

### 6.1 从 Zelland 移植到 torvox

#### 6.1.1 鼠标事件编码 (SGR 序列)

**来源**: `src-tauri/src/terminal.rs` 行 96-175

```rust
/// 编码鼠标事件为 ANSI SGR 序列
/// 适用于 torvox 的本地 PTY 写入
pub fn encode_mouse_event(
    &self,
    x_px: f32,
    y_px: f32,
    action: &str,
    cols: u16,
    rows: u16,
    cell_w: f32,
    cell_h: f32,
) -> Option<Vec<u8>> {
    let mut encoder: GhosttyMouseEncoder = std::ptr::null_mut();
    let mut event: GhosttyMouseEvent = std::ptr::null_mut();

    unsafe {
        ghostty_mouse_encoder_new(std::ptr::null(), &mut encoder);
        ghostty_mouse_encoder_setopt_from_terminal(encoder, self.term.inner);

        let size = GhosttyMouseEncoderSize {
            size: std::mem::size_of::<GhosttyMouseEncoderSize>(),
            screen_width: (cols as u32) * cell_w as u32,
            screen_height: (rows as u32) * cell_h as u32,
            cell_width: cell_w as u32,
            cell_height: cell_h as u32,
            padding_top: 0, padding_bottom: 0, padding_right: 0, padding_left: 0,
        };
        ghostty_mouse_encoder_setopt(
            encoder,
            GhosttyMouseEncoderOption_GHOSTTY_MOUSE_ENCODER_OPT_SIZE,
            &size as *const _ as *const std::ffi::c_void,
        );

        ghostty_mouse_event_new(std::ptr::null(), &mut event);
        ghostty_mouse_event_set_position(event, GhosttyMousePosition { x: x_px, y: y_px });

        let ghostty_action = match action {
            "click" | "scroll_up" | "scroll_down" => GhosttyMouseAction_GHOSTTY_MOUSE_ACTION_PRESS,
            "release" | "release_scroll_up" | "release_scroll_down" => GhosttyMouseAction_GHOSTTY_MOUSE_ACTION_RELEASE,
            _ => GhosttyMouseAction_GHOSTTY_MOUSE_ACTION_PRESS,
        };
        ghostty_mouse_event_set_action(event, ghostty_action);

        let button = match action {
            "click" | "release" => GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_LEFT,
            "right_click" => GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_RIGHT,
            "scroll_up" | "release_scroll_up" => GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_FOUR,
            "scroll_down" | "release_scroll_down" => GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_FIVE,
            _ => GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_UNKNOWN,
        };
        if button != GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_UNKNOWN {
            ghostty_mouse_event_set_button(event, button);
        } else {
            ghostty_mouse_event_clear_button(event);
        }

        ghostty_mouse_event_set_mods(event, 0);
    }

    let mut buf = [0u8; 64];
    let mut out_len: usize = 0;
    let res = unsafe {
        ghostty_mouse_encoder_encode(encoder, event, buf.as_mut_ptr() as *mut _, buf.len(), &mut out_len)
    };

    unsafe {
        ghostty_mouse_event_free(event);
        ghostty_mouse_encoder_free(encoder);
    }

    if res == GhosttyResult_GHOSTTY_SUCCESS && out_len > 0 {
        Some(buf[..out_len].to_vec())
    } else {
        None
    }
}
```

**移植建议**: torvox 已有类似的鼠标事件处理，可参考此实现添加 SGR 模式支持。

---

#### 6.1.2 捏合缩放字体大小

**来源**: `MainActivity.kt` 行 157-170

```kotlin
// 添加到 torvox 的 TerminalActivity
private lateinit var scaleDetector: ScaleGestureDetector
private var baseFontSize = 38f
private var isPinching = false

scaleDetector = ScaleGestureDetector(this, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
    override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
        isPinching = true
        return true
    }
    override fun onScale(detector: ScaleGestureDetector): Boolean {
        baseFontSize = (baseFontSize * detector.scaleFactor).coerceIn(20f, 80f)
        // 调用 JNI 更新字体大小
        TerminalBridge.setFontSizeInPlace(baseFontSize)
        return true
    }
    override fun onScaleEnd(detector: ScaleGestureDetector) {
        isPinching = false
    }
})

// 在 onTouchEvent 中
scaleDetector.onTouchEvent(event)
if (isPinching) return true
```

**移植建议**: torvox 的 `TerminalBridge.setFontSizeInPlace` 已存在，只需添加手势检测。

---

#### 6.1.3 选择操作模式 (ActionMode)

**来源**: `MainActivity.kt` 行 866-906

```kotlin
// 添加到 torvox 的终端视图
private fun startSelectionActionMode() {
    actionMode?.finish()
    actionMode = (surfaceView ?: window.decorView).startActionMode(object : ActionMode.Callback {
        override fun onCreateActionMode(mode: ActionMode, menu: Menu): Boolean {
            menu.add(0, 1, 0, android.R.string.copy).setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            menu.add(0, 2, 1, android.R.string.paste).setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            return true
        }
        override fun onPrepareActionMode(mode: ActionMode, menu: Menu) = false
        override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
            return when (item.itemId) {
                1 -> { doCopy(); mode.finish(); true }
                2 -> { doPaste(); mode.finish(); true }
                else -> false
            }
        }
        override fun onDestroyActionMode(mode: ActionMode) {
            selectionActive = false
            TerminalBridge.setSelection(0, 0, 0, 0, false)
            actionMode = null
        }
    }, ActionMode.TYPE_FLOATING)
}

private fun doCopy() {
    val text = TerminalBridge.getSelectionText(selStartCol, selStartRow, selEndCol, selEndRow)
    val cm = getSystemService(CLIPBOARD_SERVICE) as ClipboardManager
    cm.setPrimaryClip(ClipData.newPlainText("terminal", text))
}

private fun doPaste() {
    val cm = getSystemService(CLIPBOARD_SERVICE) as ClipboardManager
    val clip = cm.primaryClip ?: return
    if (clip.itemCount == 0) return
    val text = clip.getItemAt(0).coerceToText(this).toString()
    if (text.isEmpty()) return
    val bracketed = "\u001b[200~$text\u001b[201~"  // 括号粘贴模式
    TerminalBridge.writeKey(bracketed.toByteArray(Charsets.UTF_8))
}
```

**移植建议**: torvox 已有 `TerminalBridge.setSelection` 和 `TerminalBridge.getSelectionText`，只需添加 ActionMode 包装。

---

#### 6.1.4 行脏矩形缓存

**来源**: `src-tauri/src/renderer/mod.rs` 行 82-86, 842-890

```rust
// 行缓存结构
row_cache: Vec<Vec<CellRun>>,
span_buf: Vec<(String, Weight, Style, Color)>,

// 在 draw_ghostty_state 中
pub fn draw_ghostty_state(&mut self, state: &mut GhosttyRenderStateWrapper, cursor_pos: (u16, u16)) {
    let dirty = state.get_dirty();
    if dirty == GhosttyRenderStateDirty_GHOSTTY_RENDER_STATE_DIRTY_FALSE { return; }

    let mut changed = false;
    state.with_rows(|line_idx, is_dirty, cells| {
        if is_dirty || line_idx as usize >= self.row_cache.len() {
            let runs = build_row_runs(cells);
            let row_idx = line_idx as usize;
            if row_idx >= self.row_cache.len() {
                self.row_cache.resize(row_idx + 1, Vec::new());
            }
            if self.row_cache[row_idx] != runs {
                self.row_cache[row_idx] = runs;
                changed = true;
            }
        }
    });

    // 缩小时清理缓存
    let (_num_cols, num_rows) = state.get_size();
    if self.row_cache.len() > num_rows as usize {
        self.row_cache.truncate(num_rows as usize);
        changed = true;
    }

    // 无变化时跳过昂贵的文本管线更新
    if !changed { return; }

    // 构建 span_buf 并调用 set_rich_text ...
}
```

**移植建议**: torvox 的 `cell_builder.rs` 已有类似的实例化逻辑，可参考此模式优化重绘。

---

#### 6.1.5 ANSI 调色板

**来源**: `src-tauri/src/renderer/mod.rs` 行 1051-1082

```rust
/// 映射 256色调色板索引到 RGB 三元组
fn ansi_palette_color(idx: usize) -> (u8, u8, u8) {
    const PALETTE: [(u8, u8, u8); 16] = [
        (0, 0, 0), (170, 0, 0), (0, 170, 0), (170, 170, 0),
        (0, 0, 170), (170, 0, 170), (0, 170, 170), (170, 170, 170),
        (85, 85, 85), (255, 85, 85), (85, 255, 85), (255, 255, 85),
        (85, 85, 255), (255, 85, 255), (85, 255, 255), (255, 255, 255),
    ];
    if idx < 16 { return PALETTE[idx]; }
    if idx < 232 {
        // 6x6x6 色立方 (索引 16-231)
        let i = idx - 16;
        let to_level = |v: usize| if v == 0 { 0u8 } else { (55 + v * 40) as u8 };
        return (to_level(i / 36), to_level((i / 6) % 6), to_level(i % 6));
    }
    // 灰度渐变 (索引 232-255)
    let v = (8 + (idx - 232) * 10) as u8;
    (v, v, v)
}
```

**移植建议**: torvox 可能有类似的调色板实现，可参考此标准 xterm 调色板。

---

#### 6.1.6 光标 WGSL Shader

**来源**: `src-tauri/src/renderer/mod.rs` 行 19-39

```rust
const CURSOR_SHADER: &str = r#"
struct CursorUniforms {
    rect:  vec4<f32>,   // x_left, y_top, x_right, y_bottom — 全部在 NDC 空间
    color: vec4<f32>,   // r, g, b, a
}
@group(0) @binding(0) var<uniform> u: CursorUniforms;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x0 = u.rect.x; let x1 = u.rect.z;
    let y0 = u.rect.w; let y1 = u.rect.y;
    var xs = array<f32,6>(x0, x1, x0, x1, x1, x0);
    var ys = array<f32,6>(y1, y1, y0, y1, y0, y0);
    return vec4<f32>(xs[i], ys[i], 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return u.color;
}
"#;
```

**移植建议**: torvox 可能有自己的光标 shader，可参考此简洁实现。

---

### 6.2 从 wgpu-in-app 移植到 torvox

#### 6.2.1 跨平台表面创建模式

**来源**: `app-surface/src/lib.rs` 行 306-363

```rust
// torvox 可参考此模式创建表面
async fn create_iasdq_context(
    instance: Instance,
    surface: Surface<'static>,
    physical_size: (u32, u32),
) -> IASDQContext {
    let physical_size = normalize_view_size(physical_size);
    let (adapter, device, queue) = crate::request_device(&instance, &surface).await;
    let caps = surface.get_capabilities(&adapter);
    let prefered = caps.formats[0];
    
    // Android 特殊处理: 不支持 view_formats
    let view_formats = if cfg!(target_os = "android") {
        vec![format]
    } else if format.is_srgb() {
        vec![format, format.remove_srgb_suffix()]
    } else {
        vec![format.add_srgb_suffix(), format.remove_srgb_suffix()]
    };

    let mut config = surface.get_default_config(&adapter, physical_size.0, physical_size.1)
        .expect("Surface isn't supported by the adapter.");
    config.view_formats = view_formats;
    config.format = format;
    surface.configure(&device, &config);

    IASDQContext { instance, surface: SharedPtr::new(surface), config, adapter, device, queue }
}
```

**移植建议**: torvox 的 `wgpu_backend.rs` 已有类似逻辑，可参考此模式处理格式回退。

---

#### 6.2.2 触摸事件抽象

**来源**: `app-surface/src/touch.rs` 行 1-53

```rust
// torvox 可参考此模式统一触摸事件
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct StylusAngle<T> {
    pub azimuth: T,
    pub altitude: T,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum TouchPhase { Started, Moved, Ended, Cancelled }

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Touch {
    pub phase: TouchPhase,
    pub position: glam::Vec2,
    pub stylus_angle: Option<StylusAngle<f32>>,
    pub pressure: f32,
    pub major_radius: f32,
    pub interval: f32,
}
```

**移植建议**: torvox 可添加触摸压力和支持手写笔支持。

---

#### 6.2.3 表面帧获取 (错误处理)

**来源**: `app-surface/src/lib.rs` 行 215-249

```rust
// torvox 可参考此模式处理表面纹理获取错误
fn create_current_frame_view(
    &self,
    device: &wgpu::Device,
    surface: &wgpu::Surface,
    config: &wgpu::SurfaceConfiguration,
    view_format: Option<wgpu::TextureFormat>,
) -> Option<(wgpu::SurfaceTexture, wgpu::TextureView)> {
    let frame = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame)
        | wgpu::CurrentSurfaceTexture::SubOptimal(frame) => frame,
        wgpu::CurrentSurfaceTexture::Timeout
        | wgpu::CurrentSurfaceTexture::Outdated
        | wgpu::CurrentSurfaceTexture::Lost => {
            surface.configure(device, config);
            match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::SubOptimal(frame) => frame,
                _ => panic!("Failed to acquire next swap chain texture!"),
            }
        }
        wgpu::CurrentSurfaceTexture::Occluded => return None,
        wgpu::CurrentSurfaceTexture::Validation => panic!("Validation error acquiring texture"),
    };
    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("frame texture view"),
        format: if view_format.is_none() {
            Some(config.format.add_srgb_suffix())
        } else { view_format },
        ..Default::default()
    });
    Some((frame, view))
}
```

**移植建议**: torvox 的 `pass.rs` 可能有类似逻辑，可参考此全面的错误处理。

---

#### 6.2.4 尺寸归一化工具

**来源**: `app-surface/src/lib.rs` 行 71-131

```rust
// torvox 可参考此模式归一化尺寸和缩放因子
pub(crate) fn normalize_view_size(size: (u32, u32)) -> (u32, u32) {
    (size.0.max(1), size.1.max(1))
}

pub(crate) fn normalize_scale_factor(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 { scale_factor } else { 1.0 }
}

pub(crate) fn physical_size_from_logical_size(width: f32, height: f32, scale_factor: f32) -> (u32, u32) {
    let scale_factor = normalize_scale_factor(scale_factor);
    let width = if width.is_finite() && width > 0.0 { (width * scale_factor) as u32 } else { 0 };
    let height = if height.is_finite() && height > 0.0 { (height * scale_factor) as u32 } else { 0 };
    normalize_view_size((width, height))
}

pub(crate) fn normalize_touch_point(
    touch_point_x: f32, touch_point_y: f32,
    physical_size: (u32, u32), scale_factor: f32,
) -> (f32, f32) {
    let physical_size = normalize_view_size(physical_size);
    let scale_factor = normalize_scale_factor(scale_factor);
    (
        touch_point_x * scale_factor / physical_size.0 as f32,
        touch_point_y * scale_factor / physical_size.1 as f32,
    )
}
```

**移植建议**: torvox 可能有类似的 DPR 处理，可参考此模式。

---

### 6.3 架构设计模式

#### 6.3.1 事件队列模式 (torvox 已有，Zelland 可参考)

**torvox 实现**: `native/src/event.rs` 行 1-80

```rust
// torvox 的事件队列是中心化的
pub enum Event {
    Bell { session_id: u64 },
    Clipboard { session_id: u64, text: String },
    Notification { session_id: u64, title: String, body: String },
    Exit { session_id: u64, code: i32 },
    #[cfg(feature = "mcp")]
    ShowDialog { ... },
    #[cfg(feature = "mcp")]
    PickFile { ... },
    #[cfg(feature = "mcp")]
    GetClipboard { ... },
    #[cfg(feature = "mcp")]
    Timeout { ... },
}

// 全局队列
static EVENT_QUEUE: LazyLock<Mutex<VecDeque<Event>>> = ...;
```

**Zelland 差异**: 使用 Tauri Channel 发送消息，无全局队列。

**建议**: torvox 的事件队列设计更优，支持多源事件 (终端 + MCP)。

---

#### 6.3.2 会话注册表模式 (torvox 已有)

**torvox 实现**: `native/src/android/ffi.rs` 行 59-60

```rust
// torvox 使用 RwLock 注册表管理会话
static SESSION_REGISTRY: LazyLock<RwLock<HashMap<u64, Arc<Mutex<Session>>>>> = ...;
static ACTIVE_SESSION_ID: AtomicU64 = AtomicU64::new(0);
```

**Zelland 差异**: 使用 `SshManager` 结构体管理会话。

**建议**: torvox 的 `RwLock` 注册表模式更适合多会话管理。

---

#### 6.3.3 渲染器全局单例 (Zelland 模式)

**Zelland 实现**: `src-tauri/src/renderer/mod.rs` 行 107

```rust
static RENDERER: Lazy<Mutex<Option<Renderer>>> = Lazy::new(|| Mutex::new(None));
```

**torvox 差异**: 使用 `GlobalGpu` OnceLock + `Renderer` 实例。

**建议**: torvox 的 `GlobalGpu` 模式更灵活，支持多表面。

---

## 附录: 文件路径索引

### Zelland 关键文件

| 文件路径 | 作用 |
|----------|------|
| `src-tauri/src/lib.rs` | Tauri 命令入口，依赖注入 |
| `src-tauri/src/ssh.rs` | SSH 会话管理，russh 集成 |
| `src-tauri/src/terminal.rs` | 终端会话封装，鼠标事件编码 |
| `src-tauri/src/ghostty.rs` | libghostty-vt C FFI 绑定 |
| `src-tauri/src/renderer/mod.rs` | wgpu + glyphon 渲染器 |
| `src-tauri/src/renderer/android.rs` | JNI 入口点 |
| `src-tauri/src/keystore.rs` | 密钥管理 trait + 实现 |
| `src-tauri/src/network.rs` | WireGuard 隧道 |
| `src-tauri/src/helper.rs` | 远程 helper 管理 |
| `src-tauri/src/daemon.rs` | 守护进程管理 |
| `src-tauri/gen/android/app/src/main/java/com/njr/zelland/MainActivity.kt` | Android 主活动 |
| `src-tauri/gen/android/app/src/main/java/com/njr/zelland/KeybarPlugin.kt` | IME 工具栏 |
| `daemon-rs/src/server.rs` | 守护进程服务器 |
| `daemon-rs/src/main.rs` | 守护进程入口 |

### wgpu-in-app 关键文件

| 文件路径 | 作用 |
|----------|------|
| `wgpu-in-app/src/lib.rs` | 库入口，日志初始化 |
| `wgpu-in-app/src/wgpu_canvas.rs` | 画布封装，示例管理 |
| `wgpu-in-app/src/desktop.rs` | 桌面端 winit 集成 |
| `wgpu-in-app/src/ffi/android.rs` | Android JNI 桥 |
| `wgpu-in-app/src/ffi/ios.rs` | iOS FFI 桥 |
| `app-surface/src/lib.rs` | 平台抽象层，IASDQContext |
| `app-surface/src/android.rs` | Android 表面实现 |
| `app-surface/src/ios.rs` | iOS 表面实现 |
| `app-surface/src/touch.rs` | 触摸事件抽象 |
| `app-surface/src/app_surface_use_winit.rs` | winit 表面实现 |
| `cargo-so/src/main.rs` | Android SO 构建工具 |

### torvox 关键文件 (对比参考)

| 文件路径 | 作用 |
|----------|------|
| `native/src/lib.rs` | 库入口，模块声明 |
| `native/src/event.rs` | 事件类型，事件队列 |
| `native/src/android/ffi.rs` | JNI FFI 桥 |
| `native/src/terminal/session.rs` | 会话编排器 |
| `native/src/terminal/pty.rs` | PTY 管理 |
| `native/src/terminal/ghostty_terminal/` | Ghostty VT 集成 |
| `native/src/render/context.rs` | GPU 渲染器 |
| `native/src/render/pipeline.rs` | GPU 管线 |
| `native/src/render/font/` | 字体管理 |
| `native/src/render/wgpu_backend.rs` | wgpu 初始化 |
| `native/src/mcp.rs` | MCP 服务器 |

---

> **文档生成时间**: 2026-03-09
> **分析基于**: Zelland commit (当前 HEAD), wgpu-in-app commit (当前 HEAD), torvox (当前工作目录)
> **分析方法**: 静态代码分析，无推测，所有结论基于实际代码
