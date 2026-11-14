use super::render::string_to_jstring;
use super::session::read_jni_string;
use super::{
    EnvUnowned, JClass, JNI_FALSE, JNI_TRUE, JObject, JString, Ordering, ThrowRuntimeExAndFailure,
    jboolean, jbyteArray, jfloat, jfloatArray, jint, jlong, jni_str, jobjectArray, jsize, jstring,
    render_state_mut, with_render_state,
};

// ── 字体 ──────────────────────────────────────────────────
/// 空族文件列表：字族已在渲染字体库中时无需按需装入。
const NO_FAMILY_FILES: &[String] = &[];

/// 返回字体库的族名列表（fonts.xml 声明的文件集 + 用户投放目录）。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_listFontFamilies<
    'local,
>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jobjectArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        // 字体族列表：设备上取进程级族索引——它与渲染状态无关，MUST 在渲染状态锁外
        // 构造：首次构建索引要读完全部声明字体文件（真机实测 127ms），持锁构造会
        // 把渲染线程冻结同样时长（打开字体选择器时可见）。
        #[cfg(target_os = "android")]
        let families = crate::render::font::font_db::family_display_names();
        // 宿主构建的列表取自渲染字体库，须持锁取。
        #[cfg(not(target_os = "android"))]
        let families = {
            let state = render_state_mut();
            let Some(render_state) = state.as_ref() else {
                return Ok(std::ptr::null_mut());
            };
            render_state.font_pipeline.list_monospace_fonts()
        };

        let string_class = env.find_class(jni_str!("java/lang/String"));
        let Ok(string_class) = string_class else {
            return Ok(std::ptr::null_mut());
        };
        let array = env.new_object_array(families.len() as jsize, string_class, JObject::null());
        let Ok(array) = array else {
            return Ok(std::ptr::null_mut());
        };
        // 元素初值为 null：任一 `new_string` / `set_element` 失败都会留下 null，
        // 而 Kotlin 侧声明是 `Array<String>`，null 元素会在字体选择器里变成 NPE。
        // 故失败即整体作废，由 Kotlin 侧按「无数据」处理。
        for (index, family) in families.iter().enumerate() {
            let Ok(element) = env.new_string(family) else {
                log::error!("listFontFamilies: cannot encode family {family:?}, aborting list");
                return Ok(std::ptr::null_mut());
            };
            if let Err(error) = array.set_element(env, index, &element) {
                log::error!(
                    "listFontFamilies: cannot store family {family:?}, aborting list: {error}"
                );
                return Ok(std::ptr::null_mut());
            }
        }
        array.into_raw()
    })
}

/// 返回默认字体族名。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getDefaultFontName<
    'local,
>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(name) =
            with_render_state(|render_state| render_state.font_pipeline.default_font_name())
        else {
            return Ok(std::ptr::null_mut());
        };
        string_to_jstring(env, &name)
    })
}

/// 返回字体信息的打包字节（布局见本文件的 `pack_font_info`）。渲染状态尚未初始化时为 null。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getFontInfo(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(packed) = with_render_state(|render_state| {
            pack_font_info(&render_state.font_pipeline.font_info())
        }) else {
            return Ok(std::ptr::null_mut());
        };
        match env.byte_array_from_slice(&packed) {
            Ok(array) => array.into_raw(),
            Err(error) => {
                log::error!("getFontInfo: byte_array_from_slice failed: {error}");
                std::ptr::null_mut()
            }
        }
    })
}

/// 字体信息的打包头部字节数：标志 4 + 三个 `f32` 12 + 名称数 2 + CJK 名称数 2。
pub(super) const FONT_INFO_HEADER_BYTES: usize = 20;

/// 字体信息的跨语言打包布局（Kotlin 侧对应 `FontInfo` 的解码）。
///
/// ```text
/// u32  flags         bit0 主字体等宽；CJK 状态取判别序号（见 CjkFallbackState）
/// f32 ×3             单元格宽高（物理像素）、逻辑字号（sp）
/// u16  名称个数       主字体 + CJK 回退族 + 其他被使用族
/// u16  CJK 名称数     紧随其后的一段是 CJK 回退族，其余为其他被使用族
/// 名称 × n            UTF-8、NUL 分隔，先主字体族名（无主字体则空串），
///                     再 CJK 回退族名，最后其他被使用族名
/// ```
pub(crate) fn pack_font_info(info: &crate::render::font::pipeline::FontInfo) -> Vec<u8> {
    // CJK 状态占 bit1 起（bit0 留给等宽标记），与 Kotlin 的 `CjkState.entries[flags ushr 1]` 对齐。
    let mut flags = (info.cjk_state as u32) << 1;
    if info.active.as_ref().is_some_and(|active| active.monospaced) {
        flags |= 1;
    }
    let mut names = info.cjk_families.clone();
    names.extend(info.other_families.iter().cloned());
    names.insert(
        0,
        info.active
            .as_ref()
            .map_or_else(String::new, |active| active.name.clone()),
    );
    let mut packed = Vec::with_capacity(
        FONT_INFO_HEADER_BYTES + names.iter().map(|name| name.len() + 1).sum::<usize>(),
    );
    packed.extend_from_slice(&flags.to_le_bytes());
    for value in [info.cell_width_px, info.cell_height_px, info.font_size] {
        packed.extend_from_slice(&value.to_le_bytes());
    }
    // 计数用 u16 承载：族名数量一旦超出就明确报错，不静默回绕
    // （回绕会让 Kotlin 侧的「名称数与实际个数相符」校验抛异常，
    //  表现为设置页字体信息崩溃）。panic 由 JNI 守卫转成 Java 异常。
    let name_count =
        u16::try_from(names.len()).expect("font family count exceeds the u16 wire field");
    let cjk_count = u16::try_from(info.cjk_families.len())
        .expect("cjk family count exceeds the u16 wire field");
    packed.extend_from_slice(&name_count.to_le_bytes());
    packed.extend_from_slice(&cjk_count.to_le_bytes());
    for name in &names {
        packed.extend_from_slice(name.as_bytes());
        packed.push(0);
    }
    packed
}

/// 设置渲染器字体管线的字体族；找到该族时返回 true。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontFamily(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    _session_id: jlong,
    family: JString<'_>,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_FALSE, |env| {
        let Some(family_str) = read_jni_string(env, &family, "setFontFamily", "family") else {
            return Ok(JNI_FALSE);
        };
        // 字族是否已在渲染字体库中：常驻三族与用户选文件装入的面命中这条快径，
        // 无需族文件列表——族索引也就不会被构建。
        let family_resident = {
            let state = render_state_mut();
            let Some(render_state) = state.as_ref() else {
                return Ok(JNI_FALSE);
            };
            render_state.font_pipeline.family_resident(&family_str)
        };
        // 族文件列表 MUST 在渲染状态锁外解析：解析要走族索引（首次构建真机实测
        // 127ms），持锁解析会把渲染线程冻结同样时长。
        let family_files = if family_resident {
            NO_FAMILY_FILES
        } else {
            crate::render::font::font_db::family_files(&family_str).unwrap_or(NO_FAMILY_FILES)
        };
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(JNI_FALSE);
        };
        let previous = render_state.font_pipeline.font_id;
        let found = render_state
            .font_pipeline
            .set_font_family(&family_str, family_files);
        if render_state.font_pipeline.font_id != previous {
            // 字体源变化：实例缓存的图集 UV 与行高全部过期，同尺寸下仍判兼容，必须整库丢弃并重绘。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
            // 只在字面真的换了时出声：同一字族的重复下发是常态，逐次记录
            // 「未变化」只会刷屏。字族找不到的情形由管线自己告警。
            log::info!("setFontFamily: {family_str} 字面已切换");
        }
        if found { JNI_TRUE } else { JNI_FALSE }
    })
}

/// 原生接受的光栅缩放（sp→px 系数）区间。**唯一真源**。
///
/// 同一区间此前在两端各存一份字面量——Rust 的 `setRasterScale` 守卫与 Kotlin 的
/// `coerceSpToPxScale` 钳位——互相只由注释绑定。它们漂移的后果与字号上界那次
/// 完全同型：Kotlin 用被原生拒收的系数算出 `FONT_SIZE_MAX_PX / 系数` 的字号上界，
/// 而原生静默拒收该系数（记 error），滑块上界与实际渲染重新脱节。故守卫与对外
/// 查询（[`Java_terminal_emulator_bridge_NativeBridge_getRasterScaleRange`]）同取此值。
pub(super) const RASTER_SCALE_MIN: f32 = 0.5;
pub(super) const RASTER_SCALE_MAX: f32 = 8.0;

/// 缩放是否落在原生接受区间内。守卫与对外查询同走这一处判定，故「Kotlin 钳到的
/// 区间」与「原生接受的区间」在结构上就是同一个区间，不依赖两处字面量对齐。
pub(super) fn is_raster_scale_acceptable(scale: f32) -> bool {
    (RASTER_SCALE_MIN..=RASTER_SCALE_MAX).contains(&scale)
}

/// 原生接受的最大字号（sp）：字形位图必须放得进图集，故上界由图集边长推导。
///
/// 这是**唯一**的真实约束；用户可选区间由 Kotlin 侧 `SettingsRepository.fontSizeMaxSp`
/// （对标 Termux 的 256px 上限）给出，本值必须始终不小于它，否则滑块能划到的字号会被
/// 原生拒绝，用户看到的就是「设置条范围与实际可设置范围不一致」。
/// `font_size_cap_tests` 钉住这一关系。
///
/// 边长取自**管线自身**（[`FontPipeline::atlas_width`]，即实际分配的那张图集）而非本模块
/// 的 `ATLAS_SIZE` 常量：后者只是初始化时的入参，两者一旦不同步，「由图集推导」就成了
/// 假话，上界会悄悄偏到另一个尺寸上。
pub(super) fn font_size_cap_sp(atlas_edge_px: f32, raster_scale: f32) -> f32 {
    // 系数已由 `FontPipeline::get_raster_scale` 钳到 ≥ EPSILON，此处不再重复。
    atlas_edge_px / raster_scale
}

/// 字号是否落在原生可接受区间内。
pub(super) fn is_font_size_selectable(size_sp: f32, cap_sp: f32) -> bool {
    size_sp.is_finite() && size_sp > 0.0 && size_sp <= cap_sp
}

/// 设置字号（单位为十分之一 sp，与 Kotlin 滑块一致）。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontSizeInPlace(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    _session_id: jlong,
    size_tenths: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let size = (size_tenths as f32) / 10.0;
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(());
        };
        // 用户可选区间只有一处定义：`SettingsRepository.fontSizeMaxSp`（Termux 的
        // 256px 上限换算）。此前这里另有一份 `4.0..=100.0` 的魔数守卫，且对超限值
        // **静默丢弃**——Kotlin 侧那份重复常量一旦漂移，用户拖到的字号就被原生悄悄
        // 丢掉，表现正是「设置条范围和实际可设置范围不一致」。守卫上界改由图集边长
        // 推导（唯一真实约束），越界记错误日志而非无声忽略。
        let max_size = font_size_cap_sp(
            render_state.font_pipeline.atlas_width as f32,
            render_state.font_pipeline.get_raster_scale(),
        );
        if !is_font_size_selectable(size, max_size) {
            // warn 而非 error：捏合缩放的 preview 路径以动画帧率调用本函数，
            // 一次越界会刷出每帧一条 error，把真正的错误日志淹掉。
            log::warn!(
                "setFontSizeInPlace: 字号 {size} 越界（合法区间 (0, {max_size}] sp，字形位图必须放进图集）"
            );
            return Ok(());
        }
        // 同值跳过：手势 preview 高频推送同一字号时不清图集，
        // 避免每帧重光栅 ASCII + 丢实例缓存（缩放撕裂/卡顿）。
        if (render_state.font_pipeline.font_size - size).abs() < f32::EPSILON {
            return Ok(());
        }
        let (cell_width, cell_height) = render_state.font_pipeline.set_font_size_in_place(size);
        // 置脏标志：字号变化必须重绘，即便终端空闲（字形度量已变 → 缓存帧过期）。
        // 同时丢弃实例缓存：干净行存的是旧图集 UV 与旧行高，同网格尺寸仍判兼容，
        // 不丢则丢字/错位/行高混杂（压扁·撕裂）。
        render_state.renderer.cell_cache = None;
        render_state.dirty.store(true, Ordering::Relaxed);
        log::info!("setFontSizeInPlace: {size_tenths} -> cell {cell_width:.1}x{cell_height:.1}");
    });
}

/// 原生接受的光栅缩放区间 `[min, max]` → JNI `FloatArray`（原生为唯一真源）。
///
/// Kotlin 侧的 `coerceSpToPxScale` 必须钳到本区间：字号的 sp→px 换算
/// （`SettingsRepository.fontSizeMaxSp`）以该系数为分母，系数一旦落在区间外，
/// 算出的字号上界就不再对应原生真实的光栅尺度。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getRasterScaleRange<
    'local,
>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jfloatArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let bounds = [RASTER_SCALE_MIN, RASTER_SCALE_MAX];
        let Ok(array) = env.new_float_array(bounds.len()) else {
            return Ok(std::ptr::null_mut());
        };
        if array.set_region(env, 0, &bounds).is_err() {
            return Ok(std::ptr::null_mut());
        }
        array.into_raw()
    })
}

/// 设置字体光栅化缩放（设备像素密度）。字形位图按 `font_size * raster_scale`
/// 光栅化，使高密度屏上的文字保持锐利。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setRasterScale(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    _session_id: jlong,
    scale: jfloat,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        // 越界记错误日志而非静默丢弃：Kotlin 侧 `coerceSpToPxScale` 已钳到同区间
        // （该区间经 `getRasterScaleRange` 从这里取，二者是同一常量），两端常量
        // 漂移时必须在此留痕，否则字号上限换算与实际光栅尺度悄悄脱节。
        if !is_raster_scale_acceptable(scale) {
            log::error!(
                "setRasterScale: 缩放 {scale} 越界（合法区间 {RASTER_SCALE_MIN}..={RASTER_SCALE_MAX}）"
            );
            return Ok(());
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            // 缩放只落在字体管线上：字形位图按 `font_size*scale` 光栅化，
            // 单元几何（`cell_builder`）与 bearing 也按同一 scale 算，着色器一律
            // 1:1 采样。渲染器侧曾有一个同名的 uniform 字段，但它从不被着色器读取
            // （再乘一次反而会把字形缩回逻辑像素），故已删除而非同步两处。
            render_state.font_pipeline.set_raster_scale(scale);
            // 位图全部重光栅：旧 UV 指向错误图块，丢缓存并重绘。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    });
}

/// 把自定义字体文件载入渲染器的字体库。返回文件中的首个字体族名，失败时 null。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_loadFontFile<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    _session_id: jlong,
    path: JString<'local>,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(path_str) = read_jni_string(env, &path, "loadFontFile", "path") else {
            return Ok(std::ptr::null_mut());
        };
        // 自定义字体加载是 Android 专属特性（带额外路径的字体库仅 Android 有；
        // 桌面构建使用系统字体）。其他目标上拒绝加载。
        #[cfg(target_os = "android")]
        let family = {
            let mut state = render_state_mut();
            let Some(render_state) = state.as_mut() else {
                return Ok(std::ptr::null_mut());
            };
            // 直接装进当前字体库：文件路径已知，另起一个库探测族名只是把同一个
            // 文件解析两遍。装入后族已在库内，按族名选中不必枚举全部字体文件
            // （真机实测枚举 187 个族要 127ms）。
            let Some(family) = render_state
                .font_pipeline
                .load_font_file(std::path::Path::new(&path_str))
            else {
                log::warn!("loadFontFile: no family name in {path_str}");
                return Ok(std::ptr::null_mut());
            };
            // 登记到字体列表的扫描路径：追加而非覆盖，用户字体目录已在此前
            // 注册，覆盖会丢掉目录内字体。
            crate::render::font::font_db::add_extra_font_path(std::path::PathBuf::from(&path_str));
            let previous = render_state.font_pipeline.font_id;
            // 族刚装进库中：无需族文件列表（族索引不因装载字体文件而构建）。
            if !render_state
                .font_pipeline
                .set_font_family(&family, NO_FAMILY_FILES)
            {
                // 丢弃它会让设置页显示新字体名而终端仍用管线的默认字体渲染，
                // 且无任何日志（对比 setFontFamily 导出：同一结果在此被上报为 false）。
                log::error!("loadFontFile: cannot apply family {family:?} from {path_str}");
                return Ok(std::ptr::null_mut());
            }
            if render_state.font_pipeline.font_id != previous {
                // 字体源变化：实例缓存的图集 UV 与行高全部过期，同尺寸下仍判兼容，
                // 必须整库丢弃并重绘。
                render_state.renderer.cell_cache = None;
                render_state.dirty.store(true, Ordering::Relaxed);
            }
            family
        };
        #[cfg(not(target_os = "android"))]
        let family: String = {
            log::warn!("loadFontFile: unsupported on this target");
            String::new()
        };
        if family.is_empty() {
            return Ok(std::ptr::null_mut());
        }
        log::info!("loadFontFile: {path_str} -> family {family}");
        string_to_jstring(env, &family)
    })
}

/// 设置系统 locale（BCP 47，如 `zh-CN`）：决定 fonts.xml 里选哪个区域回退族。
///
/// 进程启动时 Kotlin 就会调一次（此时还没有渲染管线），故 locale 同时写入渲染层
/// 进程级静态供 `load_font_database` 取用；管线已存在时
/// 再推给它并作废同 UV 缓存（回退族变化会改变字形来源）。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSystemLocale(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    _session_id: jlong,
    locale: JString<'_>,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        let Some(locale_str) = read_jni_string(env, &locale, "setSystemLocale", "locale") else {
            return Ok(());
        };
        log::info!("setSystemLocale: {locale_str}");
        // 管线创建前的区域决策读渲染层静态（与 setExtraFontPaths 同一门控惯例）。
        #[cfg(target_os = "android")]
        crate::render::font::font_db::set_current_locale(locale_str.clone());
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.font_pipeline.set_system_locale(&locale_str);
            // CJK 回退排序变化会改变字形来源，同 UV 缓存过期。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    });
}

/// 登记额外的字体目录/文件（应用私有字体目录）：供字体列表枚举（`family_index`）
/// 与选中时按需装入（`load_family`）使用；渲染常驻字体库不读这些路径
/// （见 `load_font_database`），故不重建管线——重建会把主字体族与光栅缩放
/// 一并复位（新建管线回到默认族、缩放 1.0），用户看到字体与字号突然改变。
///
/// # Safety
/// 仅由 JVM 经 JNI 调用，`paths` 须为本次调用期间有效的 `String[]`。
#[unsafe(no_mangle)]
pub(super) unsafe extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setExtraFontPaths(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    _session_id: jlong,
    paths: jobjectArray,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        #[cfg(target_os = "android")]
        {
            // 从 Java 的 `String[]` 解析。任一项解码失败即抛异常并整体放弃：
            // 异常待决期间继续调 JNI 是未定义行为（JNI 规范只允许极少数调用），
            // 且本次调用的结果本就被该异常作废——继续读剩余项毫无意义。
            let mut path_list: Vec<std::path::PathBuf> = Vec::new();
            // SAFETY: `paths` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效
            // （与 `feed_pty_inner` 同一模式）。
            let array = unsafe { jni::objects::JObjectArray::<JString>::from_raw(env, paths) };
            // 读长度失败不得当成空数组：本次调用会**替换**已登记的路径，空列表等于
            // 把用户的 `.termux/fonts` 静默注销、字体列表随之缩水，且无任何报错。
            let count = match array.len(env) {
                Ok(count) => count,
                Err(error) => {
                    let message = jni::strings::JNIString::from(format!(
                        "setExtraFontPaths: cannot read array length: {error}"
                    ));
                    let _ = env.throw_new(jni_str!("java/lang/RuntimeException"), &message);
                    return Ok(());
                }
            };
            for font_index in 0..count {
                let Some(item) = array.get_element(env, font_index).ok() else {
                    log::warn!("setExtraFontPaths: 读不到第 {font_index} 个元素，放弃");
                    return Ok(());
                };
                let Some(text) = read_jni_string(env, &item, "setExtraFontPaths", "paths[]") else {
                    return Ok(());
                };
                path_list.push(std::path::PathBuf::from(text));
            }
            crate::render::font::font_db::set_extra_font_paths(path_list);
            log::info!("setExtraFontPaths: registered extra font paths");
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (env, paths);
            log::warn!("setExtraFontPaths: unsupported on this target");
        }
    });
}
