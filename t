[2026-10-02 17:31:11.166 Uid(value=10327):18216:18310 F/libc]
Fatal signal 6 (SIGABRT), code -1 (SI_QUEUE) in tid 18310 (DefaultDispatch), pid 18216 (com.termux)

[2026-10-02 17:31:11.877 Uid(value=10327):18351:18351 F/DEBUG]
*** *** *** *** *** *** *** *** *** *** *** *** *** *** *** ***

[2026-10-02 17:31:11.877 Uid(value=10327):18351:18351 F/DEBUG]
Build fingerprint: 'ZTE/CN_P720S20/P720S20:13/TP1A.220624.014/20240920.145249:user/release-keys'

[2026-10-02 17:31:11.877 Uid(value=10327):18351:18351 F/DEBUG]
Revision: '0'

[2026-10-02 17:31:11.877 Uid(value=10327):18351:18351 F/DEBUG]
ABI: 'arm64'

[2026-10-02 17:31:11.877 Uid(value=10327):18351:18351 F/DEBUG]
Timestamp: 2026-10-02 17:31:11.257862593+0800

[2026-10-02 17:31:11.877 Uid(value=10327):18351:18351 F/DEBUG]
Process uptime: 2s

[2026-10-02 17:31:11.877 Uid(value=10327):18351:18351 F/DEBUG]
Cmdline: com.termux

[2026-10-02 17:31:11.877 Uid(value=10327):18351:18351 F/DEBUG]
pid: 18216, tid: 18310, name: DefaultDispatch  >>> com.termux <<<

[2026-10-02 17:31:11.877 Uid(value=10327):18351:18351 F/DEBUG]
uid: 10327

[2026-10-02 17:31:11.877 Uid(value=10327):18351:18351 F/DEBUG]
signal 6 (SIGABRT), code -1 (SI_QUEUE), fault addr --------

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
    x0  0000000000000000  x1  0000000000004786  x2  0000000000000006  x3  0000006ce69f21e0

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
    x4  8399e68d97e5afa2  x5  8399e68d97e5afa2  x6  8399e68d97e5afa2  x7  ffffffffffffffff

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
    x8  00000000000000f0  x9  0000007031ecca30  x10 0000000000000001  x11 0000007031f0d5e0

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
    x12 00000000000083ee  x13 0000000000000049  x14 0000006de4684410  x15 0000000026762762

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
    x16 0000007031f7ad58  x17 0000007031f56280  x18 0000006ce2732000  x19 0000000000004728

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
    x20 0000000000004786  x21 00000000ffffffff  x22 0000000000000000  x23 0000006ce69f2678

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
    x24 0000006d7fe00880  x25 0000006ce69f41c8  x26 0000006ce69f41d8  x27 0000006ce69f41c8

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
    x28 0000006ce69f40c0  x29 0000006ce69f2260

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
    lr  0000007031efe0c8  sp  0000006ce69f21c0  pc  0000007031efe0f4  pst 0000000000001000

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
backtrace:

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #00 pc 00000000000530f4  /apex/com.android.runtime/lib64/bionic/libc.so (abort+164) (BuildId: b9cb81d165e4db3d2928d44d46481889)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #01 pc 00000000006c2754  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/lib/arm64/libnative.so

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #02 pc 00000000006bf3b8  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/lib/arm64/libnative.so

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #03 pc 000000000032e070  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/lib/arm64/libnative.so

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #04 pc 000000000032efb8  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/lib/arm64/libnative.so

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #05 pc 00000000003489a4  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/lib/arm64/libnative.so

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #06 pc 000000000031142c  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/lib/arm64/libnative.so

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #07 pc 000000000030be38  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/lib/arm64/libnative.so (Java_terminal_emulator_bridge_NativeBridge_prefetchRenderState+16)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #08 pc 000000000021a354  /apex/com.android.art/lib64/libart.so (art_quick_generic_jni_trampoline+148) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #09 pc 0000000000209398  /apex/com.android.art/lib64/libart.so (nterp_helper+152) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #10 pc 00000000001ad950  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/base.apk (terminal.emulator.bridge.Bridge$prefetchRenderStateAsync$1.invokeSuspend+20)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #11 pc 000000000020a254  /apex/com.android.art/lib64/libart.so (nterp_helper+3924) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #12 pc 000000000016d7c8  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/base.apk (kotlin.coroutines.jvm.internal.BaseContinuationImpl.resumeWith+20)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #13 pc 000000000020b120  /apex/com.android.art/lib64/libart.so (nterp_helper+7712) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #14 pc 0000000000171e38  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/base.apk (kotlinx.coroutines.DispatchedTask.run+252)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #15 pc 000000000020b074  /apex/com.android.art/lib64/libart.so (nterp_helper+7540) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #16 pc 000000000017ea1a  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/base.apk (kotlinx.coroutines.internal.LimitedDispatcher$Worker.run+6)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #17 pc 000000000020b074  /apex/com.android.art/lib64/libart.so (nterp_helper+7540) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #18 pc 0000000000181670  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/base.apk (kotlinx.coroutines.scheduling.TaskImpl.run+4)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #19 pc 000000000020b074  /apex/com.android.art/lib64/libart.so (nterp_helper+7540) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #20 pc 00000000001809e0  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/base.apk (kotlinx.coroutines.scheduling.CoroutineScheduler.runSafely+0)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #21 pc 000000000020a254  /apex/com.android.art/lib64/libart.so (nterp_helper+3924) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #22 pc 0000000000180d2a  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/base.apk (kotlinx.coroutines.scheduling.CoroutineScheduler$Worker.executeTask+66)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #23 pc 000000000020a254  /apex/com.android.art/lib64/libart.so (nterp_helper+3924) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #24 pc 0000000000181018  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/base.apk (kotlinx.coroutines.scheduling.CoroutineScheduler$Worker.runWorker+56)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #25 pc 000000000020a254  /apex/com.android.art/lib64/libart.so (nterp_helper+3924) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #26 pc 0000000000180fc8  /data/app/~~tRoYObju6IODbj4tAo22oQ==/com.termux-g7lTKsLe6czDb8cSpXn-_A==/base.apk (kotlinx.coroutines.scheduling.CoroutineScheduler$Worker.run+0)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #27 pc 000000000021096c  /apex/com.android.art/lib64/libart.so (art_quick_invoke_stub+556) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #28 pc 0000000000279de8  /apex/com.android.art/lib64/libart.so (art::ArtMethod::Invoke(art::Thread*, unsigned int*, unsigned int, art::JValue*, char const*)+184) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #29 pc 000000000062276c  /apex/com.android.art/lib64/libart.so (art::JValue art::InvokeVirtualOrInterfaceWithJValues<art::ArtMethod*>(art::ScopedObjectAccessAlreadyRunnable const&, _jobject*, art::ArtMethod*, jvalue const*)+460) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #30 pc 000000000066b020  /apex/com.android.art/lib64/libart.so (art::Thread::CreateCallback(void*)+1296) (BuildId: 7458ec6a59ee4a6b19a11231d4da3377)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #31 pc 00000000000c167c  /apex/com.android.runtime/lib64/bionic/libc.so (__pthread_start(void*)+204) (BuildId: b9cb81d165e4db3d2928d44d46481889)

[2026-10-02 17:31:11.878 Uid(value=10327):18351:18351 F/DEBUG]
      #32 pc 0000000000054930  /apex/com.android.runtime/lib64/bionic/libc.so (__start_thread+64) (BuildId: b9cb81d165e4db3d2928d44d46481889)

[2026-10-06 17:14:56.057 Uid(value=10327):27568:27568 W/Z##ZSSOSConfig]
parseXml, xmlResourceId invalid !!! xmlResourceId=0

[2026-10-06 17:14:56.057 Uid(value=10327):27568:27568 W/libc]
Access denied finding property "persist.vendor.smartslide.default_value_threshold"

[2026-10-06 17:14:56.167 Uid(value=10327):27568:27568 W/TerminalSurface]
applySurfaceResize: surface not valid yet, deferring

[2026-10-06 17:14:56.184 Uid(value=10327):27568:27568 E/SurfaceSyncer]
Failed to find sync for id=0

[2026-10-06 17:14:56.199 Uid(value=10327):27568:27618 I/native::terminal::session]
Session::spawn: shell='/data/user/0/com.termux/files/usr/bin/login', rows=24, cols=80, cwd=None

[2026-10-06 17:14:56.199 Uid(value=10327):27568:27618 I/native::terminal::pty]
SPAWN_DIRECT_ET_EXEC: shell=/data/user/0/com.termux/files/usr/bin/login (non-PIE, skip linker)

[2026-10-06 17:14:56.207 Uid(value=10327):27568:27618 I/native::terminal::session]
Session::spawn: PtyPair::spawn OK

[2026-10-06 17:14:56.207 Uid(value=10327):27568:27618 I/native::terminal::session]
Session::spawn: set_nonblocking OK

[2026-10-06 17:14:56.207 Uid(value=10327):27568:27618 I/native::terminal::session]
Session::spawn: cloning master fd for reader

[2026-10-06 17:14:56.207 Uid(value=10327):27568:27618 I/native::terminal::session]
Session::spawn_with_theme_inner: creating Arc/Channel

[2026-10-06 17:14:56.208 Uid(value=10327):27568:27618 I/native::terminal::session]
Session::spawn: spawning reader thread

[2026-10-06 17:14:56.208 Uid(value=10327):27568:27618 I/native::android::ffi]
FFI: initSession -> id=1

[2026-10-06 17:14:56.209 Uid(value=10327):27568:27624 I/native::terminal::session]
wait thread: waiting for child pid=27621

[2026-10-06 17:14:56.209 Uid(value=10327):27621:27621 W/DefaultDispatch]
type=1400 audit(0.0:2091177): avc: granted { execute } for name="login" dev="dm-38" ino=6573816 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:14:56.213 Uid(value=10327):27621:27621 W/DefaultDispatch]
type=1400 audit(0.0:2091178): avc: granted { execute_no_trans } for path="/data/user/0/com.termux/files/usr/bin/login" dev="dm-38" ino=6573816 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:14:56.217 Uid(value=10327):27621:27621 W/login]
type=1400 audit(0.0:2091179): avc: granted { execute } for path="/data/user/0/com.termux/files/usr/bin/login" dev="dm-38" ino=6573816 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:14:56.232 Uid(value=10327):27568:27597 I/mali_gralloc]
register: id=0x2990002189d, importpid=-1

[2026-10-06 17:14:56.235 Uid(value=10327):27568:27593 I/native::render::wgpu_backend]
GPU adapter: Mali-G57 (backend=Vulkan, type=IntegratedGpu)

[2026-10-06 17:14:56.235 Uid(value=10327):27568:27593 W/wgpu_core::instance]
Missing downlevel flags: DownlevelFlags(SURFACE_VIEW_FORMATS)
The underlying API or device in use does not support enough features to be a fully compliant implementation of WebGPU. A subset of the features can still be used. If you are running this program on native and not in a browser and wish to limit the features you use to the supported subset, call Adapter::downlevel_properties or Device::downlevel_properties to get a listing of the features the current platform supports.

[2026-10-06 17:14:56.235 Uid(value=10327):27568:27593 W/wgpu_core::instance]
DownlevelCapabilities {
    flags: DownlevelFlags(
        COMPUTE_SHADERS | FRAGMENT_WRITABLE_STORAGE | INDIRECT_EXECUTION | BASE_VERTEX | READ_ONLY_DEPTH_STENCIL | NON_POWER_OF_TWO_MIPMAPPED_TEXTURES | CUBE_ARRAY_TEXTURES | COMPARISON_SAMPLERS | INDEPENDENT_BLEND | VERTEX_STORAGE | ANISOTROPIC_FILTERING | FRAGMENT_STORAGE | MULTISAMPLED_SHADING | DEPTH_TEXTURE_AND_BUFFER_COPIES | WEBGPU_TEXTURE_FORMAT_SUPPORT | BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED | UNRESTRICTED_INDEX_BUFFER | FULL_DRAW_INDEX_UINT32 | DEPTH_BIAS_CLAMP | VIEW_FORMATS | UNRESTRICTED_EXTERNAL_TEXTURE_COPIES | NONBLOCKING_QUERY_RESOLVE | SHADER_F16_IN_F32 | MSL2_1 | TEXTURE_COMPRESSION,
    ),
    limits: DownlevelLimits,
    shader_model: Sm5,
}

[2026-10-06 17:14:56.253 Uid(value=10327):27568:27618 I/native::render::wgpu_backend]
GPU adapter: Mali-G57 (backend=Vulkan, type=IntegratedGpu)

[2026-10-06 17:14:56.253 Uid(value=10327):27568:27618 W/wgpu_core::instance]
Missing downlevel flags: DownlevelFlags(SURFACE_VIEW_FORMATS)
The underlying API or device in use does not support enough features to be a fully compliant implementation of WebGPU. A subset of the features can still be used. If you are running this program on native and not in a browser and wish to limit the features you use to the supported subset, call Adapter::downlevel_properties or Device::downlevel_properties to get a listing of the features the current platform supports.

[2026-10-06 17:14:56.253 Uid(value=10327):27568:27618 W/wgpu_core::instance]
DownlevelCapabilities {
    flags: DownlevelFlags(
        COMPUTE_SHADERS | FRAGMENT_WRITABLE_STORAGE | INDIRECT_EXECUTION | BASE_VERTEX | READ_ONLY_DEPTH_STENCIL | NON_POWER_OF_TWO_MIPMAPPED_TEXTURES | CUBE_ARRAY_TEXTURES | COMPARISON_SAMPLERS | INDEPENDENT_BLEND | VERTEX_STORAGE | ANISOTROPIC_FILTERING | FRAGMENT_STORAGE | MULTISAMPLED_SHADING | DEPTH_TEXTURE_AND_BUFFER_COPIES | WEBGPU_TEXTURE_FORMAT_SUPPORT | BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED | UNRESTRICTED_INDEX_BUFFER | FULL_DRAW_INDEX_UINT32 | DEPTH_BIAS_CLAMP | VIEW_FORMATS | UNRESTRICTED_EXTERNAL_TEXTURE_COPIES | NONBLOCKING_QUERY_RESOLVE | SHADER_F16_IN_F32 | MSL2_1 | TEXTURE_COMPRESSION,
    ),
    limits: DownlevelLimits,
    shader_model: Sm5,
}

[2026-10-06 17:14:56.267 Uid(value=10327):27568:27618 I/native::render::wgpu_backend]
GPU device created, queue ok

[2026-10-06 17:14:56.265 Uid(value=10327):27621:27621 W/login]
type=1400 audit(0.0:2091180): avc: denied { getattr } for path="/proc/modules" dev="proc" ino=4026532130 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:proc_modules:s0 tclass=file permissive=0 app=com.termux

[2026-10-06 17:14:56.265 Uid(value=10327):27621:27621 W/login]
type=1400 audit(0.0:2091181): avc: denied { getattr } for path="/proc/bus/pci/devices" dev="proc" ino=4026532146 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:proc:s0 tclass=file permissive=0 app=com.termux

[2026-10-06 17:14:56.290 Uid(value=10327):27568:27593 I/native::render::wgpu_backend]
GPU device created, queue ok

[2026-10-06 17:14:56.291 Uid(value=10327):27568:27618 W/native::render::font::cjk]
CJK_FALLBACK: fonts.xml 未提供匹配当前语言的回退字体

[2026-10-06 17:14:56.291 Uid(value=10327):27568:27597 W/Parcel]
Expecting binder but got null!

[2026-10-06 17:14:56.331 Uid(value=10327):27568:27618 I/native::android::ffi]
render state initialized (renderer + font pipeline)

[2026-10-06 17:14:56.336 Uid(value=10327):27568:27618 W/native::render::font::cjk]
CJK_FALLBACK: fonts.xml 未提供匹配当前语言的回退字体

[2026-10-06 17:14:56.343 Uid(value=10327):27568:27568 I/Choreographer]
Skipped 20 frames by ZTE_SUPERKILLER_SKIPPED_FRAME_GAUGE, going to report to SuperKiller

[2026-10-06 17:14:56.352 Uid(value=10327):27568:27618 I/native::android::ffi]
setExtraFontPaths: registered extra font paths

[2026-10-06 17:14:56.352 Uid(value=10327):27568:27593 I/native::android::ffi]
render state prefetched

[2026-10-06 17:14:56.352 Uid(value=10327):27568:27618 I/native::android::ffi]
setSystemLocale: zh-CN

[2026-10-06 17:14:56.381 Uid(value=10327):27568:27568 W/MemoryMonitor]
TRIM_MEMORY_RUNNING_MODERATE

[2026-10-06 17:14:56.381 Uid(value=10327):27568:27597 I/mali_gralloc]
register: id=0x2990002189e, importpid=-1

[2026-10-06 17:14:56.383 Uid(value=10327):27568:27568 W/libc]
Access denied finding property "persist.unipnp.debug"

[2026-10-06 17:14:56.383 Uid(value=10327):27568:27568 W/libc]
Access denied finding property "persist.unipnp.debug"

[2026-10-06 17:14:56.383 Uid(value=10327):27568:27568 W/ziparchive]
Unable to open '/system_ext/framework/unipnp-framework.dm': No such file or directory

[2026-10-06 17:14:56.383 Uid(value=10327):27568:27568 W/ziparchive]
Unable to open '/system_ext/framework/unipnp-framework.dm': No such file or directory

[2026-10-06 17:14:56.386 Uid(value=10327):27568:27618 I/native::android::ffi]
setFontFamily: Droid Sans Mono found=true

[2026-10-06 17:14:56.392 Uid(value=10327):27568:27618 I/native::android::ffi]
setTheme: session 1 background=[21, 21, 21] foreground=[F8, F8, F2]

[2026-10-06 17:14:56.392 Uid(value=10327):27568:27618 I/native::android::ffi]
setCursorColor: (0.9254902, 0.9372549, 0.95686275)

[2026-10-06 17:14:56.393 Uid(value=10327):27568:27618 I/native::android::ffi]
FFI: attachWindow ptr=0x6f046aa9e0 480x819

[2026-10-06 17:14:56.404 Uid(value=10327):27568:27618 W/libc]
Access denied finding property "persist.vendor.gpu.fbc"

[2026-10-06 17:14:56.404 Uid(value=10327):27568:27618 I/mali_gralloc]
SPRD: get persist.vendor.gpu.fbc 1

[2026-10-06 17:14:56.404 Uid(value=10327):27568:27618 E/mali_gralloc]
Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator

[2026-10-06 17:14:56.404 Uid(value=10327):27568:27618 E/mali_gralloc]
ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00

[2026-10-06 17:14:56.404 Uid(value=10327):27568:27618 E/mali_gralloc]
Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator

[2026-10-06 17:14:56.404 Uid(value=10327):27568:27618 E/mali_gralloc]
ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00

[2026-10-06 17:14:56.405 Uid(value=10327):27568:27618 E/mali_gralloc]
Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator

[2026-10-06 17:14:56.405 Uid(value=10327):27568:27618 E/mali_gralloc]
ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00

[2026-10-06 17:14:56.405 Uid(value=10327):27568:27618 E/mali_gralloc]
Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator

[2026-10-06 17:14:56.405 Uid(value=10327):27568:27618 E/mali_gralloc]
ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00

[2026-10-06 17:14:56.406 Uid(value=10327):27568:27618 I/mali_gralloc]
register: id=0x2990002189f, importpid=-1

[2026-10-06 17:14:56.407 Uid(value=10327):27568:27618 I/mali_gralloc]
register: id=0x299000218a0, importpid=-1

[2026-10-06 17:14:56.409 Uid(value=10327):27568:27618 I/mali_gralloc]
register: id=0x299000218a1, importpid=-1

[2026-10-06 17:14:56.410 Uid(value=10327):27568:27618 I/mali_gralloc]
register: id=0x299000218a2, importpid=-1

[2026-10-06 17:14:56.445 Uid(value=10327):27568:27618 I/mali_gralloc]
register: id=0x299000218a3, importpid=-1

[2026-10-06 17:14:56.446 Uid(value=10327):27568:27618 I/native::render::context]
ensure_frame_texture: creating accumulator 480x819 (Rgba8Unorm)

[2026-10-06 17:14:56.450 Uid(value=10327):27568:27618 I/native::render::context]
attach_surface: configured 480x819

[2026-10-06 17:14:56.450 Uid(value=10327):27568:27618 I/native::android::ffi]
FFI: attachWindow surface attached (session 1)

[2026-10-06 17:14:56.466 Uid(value=10327):27568:27593 I/native::android::ffi]
setFontFamily: Droid Sans Mono found=true

[2026-10-06 17:14:56.499 Uid(value=10327):27568:27670 I/native::render::context]
initialize_pipeline_and_bind_group: pipeline=true atlas=2048x2048 surface=480x819

[2026-10-06 17:14:56.535 Uid(value=10327):27568:27618 I/native::android::ffi]
setFontFamily: Droid Sans Mono found=true

[2026-10-06 17:14:56.535 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=68.236423 count=1 newOutput=false scrollOffset=0

[2026-10-06 17:14:57.292 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=33.411885 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:14:57.413 Uid(value=10327):27568:27597 I/mali_gralloc]
register: id=0x299000218a7, importpid=-1

[2026-10-06 17:14:57.495 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=2ms p95=15ms max=68ms scrollback=0 rows

[2026-10-06 17:14:57.496 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=17ms p95=32ms max=97ms ≈58fps

[2026-10-06 17:14:57.546 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=50.071154 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:14:58.598 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=1ms p95=0ms max=50ms scrollback=0 rows

[2026-10-06 17:14:58.598 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=18ms p95=17ms max=102ms ≈55fps

[2026-10-06 17:14:59.054 Uid(value=10327):27568:27576 I/com.termux]
Compiler allocated 4260KB to compile java.lang.Object terminal.emulator.runtime.TerminalRuntime$RenderSupervisor$startRenderThread$renderThread$1$1.invokeSuspend(java.lang.Object)

[2026-10-06 17:14:59.633 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=2ms scrollback=0 rows

[2026-10-06 17:14:59.633 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=17ms p95=17ms max=101ms ≈58fps

[2026-10-06 17:15:00.619 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=4ms scrollback=0 rows

[2026-10-06 17:15:00.619 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=17ms max=102ms ≈62fps

[2026-10-06 17:15:01.604 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=3ms scrollback=0 rows

[2026-10-06 17:15:01.604 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=17ms max=84ms ≈62fps

[2026-10-06 17:15:01.637 Uid(value=10327):27568:27576 I/com.termux]
Compiler allocated 5247KB to compile java.lang.Object terminal.emulator.runtime.TerminalRuntime$RenderSupervisor$startRenderThread$renderThread$1$1.invokeSuspend(java.lang.Object)

[2026-10-06 17:15:02.573 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=2ms scrollback=0 rows

[2026-10-06 17:15:02.574 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=17ms max=68ms ≈62fps

[2026-10-06 17:15:03.543 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=3ms scrollback=0 rows

[2026-10-06 17:15:03.543 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=17ms max=102ms ≈62fps

[2026-10-06 17:15:03.821 Uid(value=10327):27661:27661 W/bash]
type=1400 audit(0.0:2091209): avc: granted { execute } for name="nix" dev="dm-38" ino=8587204 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:03.825 Uid(value=10327):27621:27621 W/proot]
type=1400 audit(0.0:2091210): avc: granted { execute } for name="nix" dev="dm-38" ino=8587204 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:03.825 Uid(value=10327):27621:27621 W/proot]
type=1400 audit(0.0:2091211): avc: granted { execute } for name="ld-linux-aarch64.so.1" dev="dm-38" ino=8586395 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:03.825 Uid(value=10327):27689:27689 W/bash]
type=1400 audit(0.0:2091212): avc: granted { execute } for name="prooted-27621-KhlaEe" dev="dm-38" ino=6643109 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:03.825 Uid(value=10327):27689:27689 W/bash]
type=1400 audit(0.0:2091213): avc: granted { execute_no_trans } for path="/data/data/com.termux/files/usr/tmp/prooted-27621-KhlaEe" dev="dm-38" ino=6643109 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:03.879 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=36.951653 count=1 newOutput=true scrollOffset=0

[2026-10-06 17:15:04.145 Uid(value=10327):27568:27576 I/com.termux]
Compiler allocated 4260KB to compile java.lang.Object terminal.emulator.runtime.TerminalRuntime$RenderSupervisor$startRenderThread$renderThread$1$1.invokeSuspend(java.lang.Object)

[2026-10-06 17:15:04.343 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=33.577731 count=1 newOutput=true scrollOffset=0

[2026-10-06 17:15:04.546 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=1ms p95=0ms max=36ms scrollback=1 rows

[2026-10-06 17:15:04.547 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=36ms max=69ms ≈62fps

[2026-10-06 17:15:05.450 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=37.867538 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:05.520 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=37ms scrollback=1 rows

[2026-10-06 17:15:05.520 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=17ms max=68ms ≈62fps

[2026-10-06 17:15:05.576 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=56.225962 count=1 newOutput=true scrollOffset=0

[2026-10-06 17:15:06.499 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=2ms p95=20ms max=56ms scrollback=1 rows

[2026-10-06 17:15:06.499 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=37ms max=101ms ≈62fps

[2026-10-06 17:15:06.530 Uid(value=10327):27568:27576 I/com.termux]
Compiler allocated 5247KB to compile java.lang.Object terminal.emulator.runtime.TerminalRuntime$RenderSupervisor$startRenderThread$renderThread$1$1.invokeSuspend(java.lang.Object)

[2026-10-06 17:15:07.471 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=1ms p95=17ms max=24ms scrollback=4 rows

[2026-10-06 17:15:07.471 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=35ms max=105ms ≈62fps

[2026-10-06 17:15:07.889 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=39.193115 count=1 newOutput=true scrollOffset=0

[2026-10-06 17:15:08.403 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=1ms p95=3ms max=39ms scrollback=4 rows

[2026-10-06 17:15:08.403 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=15ms p95=22ms max=85ms ≈66fps

[2026-10-06 17:15:09.328 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=39.943154 count=1 newOutput=true scrollOffset=0

[2026-10-06 17:15:09.422 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=1ms p95=0ms max=39ms scrollback=4 rows

[2026-10-06 17:15:09.423 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=18ms max=152ms ≈62fps

[2026-10-06 17:15:10.426 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=0ms scrollback=4 rows

[2026-10-06 17:15:10.426 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=17ms max=85ms ≈62fps

[2026-10-06 17:15:10.450 Uid(value=10327):27568:27576 I/com.termux]
Compiler allocated 4260KB to compile java.lang.Object terminal.emulator.runtime.TerminalRuntime$RenderSupervisor$startRenderThread$renderThread$1$1.invokeSuspend(java.lang.Object)

[2026-10-06 17:15:11.315 Uid(value=10327):27568:27568 I/native::android::ffi]
setRenderPaused: paused=true

[2026-10-06 17:15:11.355 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=37.394308 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:11.481 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=1ms p95=5ms max=37ms scrollback=4 rows

[2026-10-06 17:15:11.481 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=17ms p95=35ms max=122ms ≈58fps

[2026-10-06 17:15:12.421 Uid(value=10327):27568:27568 I/native::android::ffi]
setRenderPaused: paused=false

[2026-10-06 17:15:12.536 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=22ms scrollback=4 rows

[2026-10-06 17:15:12.536 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=17ms p95=17ms max=105ms ≈58fps

[2026-10-06 17:15:13.145 Uid(value=10327):27568:27576 I/com.termux]
Compiler allocated 5247KB to compile java.lang.Object terminal.emulator.runtime.TerminalRuntime$RenderSupervisor$startRenderThread$renderThread$1$1.invokeSuspend(java.lang.Object)

[2026-10-06 17:15:13.545 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=0ms scrollback=4 rows

[2026-10-06 17:15:13.545 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=16ms p95=17ms max=98ms ≈62fps

[2026-10-06 17:15:14.986 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=0ms p95=0ms max=19ms scrollback=4 rows

[2026-10-06 17:15:14.986 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=23ms p95=17ms max=453ms ≈43fps

[2026-10-06 17:15:15.037 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=50.912231 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:16.041 Uid(value=10327):27568:27670 I/Runtime]
session 1 frame timing window (60 frames): avg=1ms p95=0ms max=50ms scrollback=4 rows

[2026-10-06 17:15:16.041 Uid(value=10327):27568:27670 I/Runtime]
session 1 loop timing window (60 frames): avg=17ms p95=17ms max=88ms ≈58fps

[2026-10-06 17:15:16.816 Uid(value=10327):27568:27600 I/native::terminal::session]
Session::spawn: shell='/data/user/0/com.termux/files/usr/bin/login', rows=24, cols=80, cwd=None

[2026-10-06 17:15:16.816 Uid(value=10327):27568:27600 I/native::terminal::pty]
SPAWN_DIRECT_ET_EXEC: shell=/data/user/0/com.termux/files/usr/bin/login (non-PIE, skip linker)

[2026-10-06 17:15:16.827 Uid(value=10327):27568:27600 I/native::terminal::session]
Session::spawn: PtyPair::spawn OK

[2026-10-06 17:15:16.827 Uid(value=10327):27568:27600 I/native::terminal::session]
Session::spawn: set_nonblocking OK

[2026-10-06 17:15:16.827 Uid(value=10327):27568:27600 I/native::terminal::session]
Session::spawn: cloning master fd for reader

[2026-10-06 17:15:16.827 Uid(value=10327):27568:27600 I/native::terminal::session]
Session::spawn_with_theme_inner: creating Arc/Channel

[2026-10-06 17:15:16.828 Uid(value=10327):27568:27600 I/native::terminal::session]
Session::spawn: spawning reader thread

[2026-10-06 17:15:16.828 Uid(value=10327):27568:27600 I/native::android::ffi]
FFI: initSession -> id=2

[2026-10-06 17:15:16.828 Uid(value=10327):27568:27600 I/native::android::ffi]
setTheme: session 2 background=[21, 21, 21] foreground=[F8, F8, F2]

[2026-10-06 17:15:16.829 Uid(value=10327):27568:27600 I/native::android::ffi]
setCursorColor: (0.9254902, 0.9372549, 0.95686275)

[2026-10-06 17:15:16.829 Uid(value=10327):27568:27617 I/native::android::ffi]
render state prefetched

[2026-10-06 17:15:16.829 Uid(value=10327):27568:27600 I/native::android::ffi]
setSystemLocale: zh-CN

[2026-10-06 17:15:16.829 Uid(value=10327):27568:27722 I/native::terminal::session]
wait thread: waiting for child pid=27719

[2026-10-06 17:15:16.829 Uid(value=10327):27719:27719 W/DefaultDispatch]
type=1400 audit(0.0:2091317): avc: granted { execute } for name="login" dev="dm-38" ino=6573816 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:16.829 Uid(value=10327):27719:27719 W/DefaultDispatch]
type=1400 audit(0.0:2091318): avc: granted { execute_no_trans } for path="/data/user/0/com.termux/files/usr/bin/login" dev="dm-38" ino=6573816 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:16.839 Uid(value=10327):27568:27600 W/native::render::font::cjk]
CJK_FALLBACK: fonts.xml 未提供匹配当前语言的回退字体

[2026-10-06 17:15:16.841 Uid(value=10327):27719:27719 W/login]
type=1400 audit(0.0:2091319): avc: granted { execute } for path="/data/user/0/com.termux/files/usr/bin/login" dev="dm-38" ino=6573816 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:16.845 Uid(value=10327):27719:27719 W/login]
type=1400 audit(0.0:2091320): avc: denied { getattr } for path="/proc/modules" dev="proc" ino=4026532130 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:proc_modules:s0 tclass=file permissive=0 app=com.termux

[2026-10-06 17:15:16.845 Uid(value=10327):27719:27719 W/login]
type=1400 audit(0.0:2091321): avc: denied { getattr } for path="/proc/bus/pci/devices" dev="proc" ino=4026532146 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:proc:s0 tclass=file permissive=0 app=com.termux

[2026-10-06 17:15:16.857 Uid(value=10327):27568:27600 I/native::android::ffi]
setExtraFontPaths: registered extra font paths

[2026-10-06 17:15:16.880 Uid(value=10327):27568:27600 W/native::render::font::cjk]
CJK_FALLBACK: fonts.xml 未提供匹配当前语言的回退字体

[2026-10-06 17:15:16.880 Uid(value=10327):27568:27600 I/native::android::ffi]
setFontFamily: Droid Sans Mono found=true

[2026-10-06 17:15:16.905 Uid(value=10327):27568:27670 W/Runtime]
SLOW_FRAME session=1 render=67.436192 count=1 newOutput=false scrollOffset=0

[2026-10-06 17:15:16.914 Uid(value=10327):27568:27600 I/native::android::ffi]
FFI: attachWindow ptr=0x6f046aa9e0 480x819

[2026-10-06 17:15:16.914 Uid(value=10327):27568:27600 I/native::render::context]
attach_surface: RECONFIGURE_SWAPCHAIN (fast path, existing surface)

[2026-10-06 17:15:16.914 Uid(value=10327):27568:27600 I/native::android::ffi]
FFI: attachWindow surface attached (session 2)

[2026-10-06 17:15:16.947 Uid(value=10327):27568:27617 W/native::render::font::cjk]
CJK_FALLBACK: fonts.xml 未提供匹配当前语言的回退字体

[2026-10-06 17:15:16.947 Uid(value=10327):27568:27617 I/native::android::ffi]
setFontFamily: Droid Sans Mono found=true

[2026-10-06 17:15:17.043 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=106.020039 count=1 newOutput=false scrollOffset=0

[2026-10-06 17:15:17.121 Uid(value=10327):27568:27617 W/native::render::font::cjk]
CJK_FALLBACK: fonts.xml 未提供匹配当前语言的回退字体

[2026-10-06 17:15:17.121 Uid(value=10327):27568:27617 I/native::android::ffi]
setFontFamily: Droid Sans Mono found=true

[2026-10-06 17:15:17.992 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=2ms p95=0ms max=106ms scrollback=0 rows

[2026-10-06 17:15:17.992 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=17ms p95=28ms max=106ms ≈58fps

[2026-10-06 17:15:18.343 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=33.780423 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:18.444 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=43.74827 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:18.835 Uid(value=10327):27568:27618 I/native::android::ffi]
FFI: destroySession id=1

[2026-10-06 17:15:18.835 Uid(value=10327):27568:27624 I/native::terminal::session]
wait thread: child exited: Ok(Signaled(Pid(27621), SIGKILL, false))

[2026-10-06 17:15:18.835 Uid(value=10327):27568:27623 I/native::terminal::session]
reader thread: output channel closed

[2026-10-06 17:15:18.829 Uid(value=10327):27661:27661 W/bash]
type=1400 audit(0.0:2091347): avc: denied { ioctl } for path="/dev/pts/0" dev="devpts" ino=3 ioctlcmd=0x540a scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:untrusted_app_all_devpts:s0:c71,c257,c512,c768 tclass=chr_file permissive=0 app=com.termux

[2026-10-06 17:15:18.829 Uid(value=10327):27661:27661 W/bash]
type=1400 audit(0.0:2091348): avc: denied { ioctl } for path="/dev/pts/0" dev="devpts" ino=3 ioctlcmd=0x542c scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:untrusted_app_all_devpts:s0:c71,c257,c512,c768 tclass=chr_file permissive=0 app=com.termux

[2026-10-06 17:15:18.829 Uid(value=10327):27661:27661 W/bash]
type=1400 audit(0.0:2091349): avc: denied { ioctl } for path="/dev/pts/0" dev="devpts" ino=3 ioctlcmd=0x542c scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:untrusted_app_all_devpts:s0:c71,c257,c512,c768 tclass=chr_file permissive=0 app=com.termux

[2026-10-06 17:15:18.947 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=37.841307 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:18.959 Uid(value=10327):27568:27618 W/native::render::font::cjk]
CJK_FALLBACK: fonts.xml 未提供匹配当前语言的回退字体

[2026-10-06 17:15:18.959 Uid(value=10327):27568:27618 I/native::android::ffi]
setFontFamily: Droid Sans Mono found=true

[2026-10-06 17:15:19.051 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=92.105423 count=1 newOutput=false scrollOffset=0

[2026-10-06 17:15:19.148 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=3ms p95=33ms max=92ms scrollback=0 rows

[2026-10-06 17:15:19.148 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=19ms p95=51ms max=104ms ≈52fps

[2026-10-06 17:15:20.203 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=0ms scrollback=0 rows

[2026-10-06 17:15:20.203 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=17ms p95=17ms max=107ms ≈58fps

[2026-10-06 17:15:20.254 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=50.430461 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:21.460 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=1ms p95=0ms max=50ms scrollback=0 rows

[2026-10-06 17:15:21.460 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=20ms p95=17ms max=295ms ≈50fps

[2026-10-06 17:15:21.564 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=33.576347 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:21.626 Uid(value=10327):27568:27568 I/Choreographer]
Skipped 26 frames by ZTE_SUPERKILLER_SKIPPED_FRAME_GAUGE, going to report to SuperKiller

[2026-10-06 17:15:21.641 Uid(value=10327):27568:27746 I/mali_gralloc]
register: id=0x299000218b5, importpid=-1

[2026-10-06 17:15:21.641 Uid(value=10327):27568:27746 I/mali_gralloc]
register: id=0x299000218b6, importpid=-1

[2026-10-06 17:15:21.644 Uid(value=10327):27568:27746 I/mali_gralloc]
register: id=0x299000218b7, importpid=-1

[2026-10-06 17:15:21.653 Uid(value=10327):27568:27597 W/Parcel]
Expecting binder but got null!

[2026-10-06 17:15:21.661 Uid(value=10327):27568:27749 I/mali_gralloc]
register: id=0x299000218b8, importpid=-1

[2026-10-06 17:15:21.661 Uid(value=10327):27568:27749 I/mali_gralloc]
register: id=0x299000218b9, importpid=-1

[2026-10-06 17:15:21.662 Uid(value=10327):27568:27749 I/mali_gralloc]
register: id=0x299000218ba, importpid=-1

[2026-10-06 17:15:21.663 Uid(value=10327):27568:27597 W/Parcel]
Expecting binder but got null!

[2026-10-06 17:15:21.665 Uid(value=10327):27568:27597 I/mali_gralloc]
unregister: id=0x299000218b7, base=0x0, importpid=27568

[2026-10-06 17:15:21.665 Uid(value=10327):27568:27597 I/mali_gralloc]
unregister: id=0x299000218b6, base=0x0, importpid=27568

[2026-10-06 17:15:21.669 Uid(value=10327):27568:27674 I/mali_gralloc]
unregister: id=0x299000218b5, base=0x0, importpid=27568

[2026-10-06 17:15:21.717 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=34.723153 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:21.718 Uid(value=10327):27568:27597 I/mali_gralloc]
unregister: id=0x299000218ba, base=0x0, importpid=27568

[2026-10-06 17:15:21.718 Uid(value=10327):27568:27597 I/mali_gralloc]
unregister: id=0x299000218b9, base=0x0, importpid=27568

[2026-10-06 17:15:21.718 Uid(value=10327):27568:27568 I/mali_gralloc]
unregister: id=0x299000218b8, base=0x0, importpid=27568

[2026-10-06 17:15:21.739 Uid(value=10327):27568:27751 I/mali_gralloc]
register: id=0x299000218bb, importpid=-1

[2026-10-06 17:15:21.739 Uid(value=10327):27568:27751 I/mali_gralloc]
register: id=0x299000218bc, importpid=-1

[2026-10-06 17:15:21.740 Uid(value=10327):27568:27751 I/mali_gralloc]
register: id=0x299000218bd, importpid=-1

[2026-10-06 17:15:21.743 Uid(value=10327):27568:27597 W/Parcel]
Expecting binder but got null!

[2026-10-06 17:15:21.752 Uid(value=10327):27568:27752 I/mali_gralloc]
register: id=0x299000218be, importpid=-1

[2026-10-06 17:15:21.753 Uid(value=10327):27568:27752 I/mali_gralloc]
register: id=0x299000218bf, importpid=-1

[2026-10-06 17:15:21.753 Uid(value=10327):27568:27752 I/mali_gralloc]
register: id=0x299000218c0, importpid=-1

[2026-10-06 17:15:21.754 Uid(value=10327):27568:27597 W/Parcel]
Expecting binder but got null!

[2026-10-06 17:15:22.197 Uid(value=10327):27568:27597 I/mali_gralloc]
unregister: id=0x299000218bf, base=0x0, importpid=27568

[2026-10-06 17:15:22.197 Uid(value=10327):27568:27597 I/mali_gralloc]
unregister: id=0x299000218be, base=0x0, importpid=27568

[2026-10-06 17:15:22.197 Uid(value=10327):27568:27568 I/mali_gralloc]
unregister: id=0x299000218c0, base=0x0, importpid=27568

[2026-10-06 17:15:22.248 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=45.576077 count=1 newOutput=false scrollOffset=0

[2026-10-06 17:15:22.275 Uid(value=10327):27568:27597 I/mali_gralloc]
unregister: id=0x299000218bc, base=0x0, importpid=27568

[2026-10-06 17:15:22.275 Uid(value=10327):27568:27597 I/mali_gralloc]
unregister: id=0x299000218bb, base=0x0, importpid=27568

[2026-10-06 17:15:22.276 Uid(value=10327):27568:27568 I/mali_gralloc]
unregister: id=0x299000218bd, base=0x0, importpid=27568

[2026-10-06 17:15:22.281 Uid(value=10327):27568:27568 W/InputEventReceiver]
Attempted to finish an input event but the input event receiver has already been disposed.

[2026-10-06 17:15:22.498 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=3ms p95=26ms max=45ms scrollback=0 rows

[2026-10-06 17:15:22.498 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=17ms p95=45ms max=80ms ≈58fps

[2026-10-06 17:15:23.472 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=2ms max=18ms scrollback=0 rows

[2026-10-06 17:15:23.472 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=16ms p95=17ms max=84ms ≈62fps

[2026-10-06 17:15:23.523 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=50.911577 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:24.475 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=50ms scrollback=0 rows

[2026-10-06 17:15:24.476 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=16ms p95=17ms max=70ms ≈62fps

[2026-10-06 17:15:25.429 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=0ms scrollback=0 rows

[2026-10-06 17:15:25.429 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=15ms p95=17ms max=88ms ≈66fps

[2026-10-06 17:15:25.780 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=38.754615 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:26.334 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=38ms scrollback=0 rows

[2026-10-06 17:15:26.334 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=15ms p95=17ms max=89ms ≈66fps

[2026-10-06 17:15:27.339 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=0ms scrollback=0 rows

[2026-10-06 17:15:27.339 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=16ms p95=17ms max=77ms ≈62fps

[2026-10-06 17:15:28.291 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=0ms scrollback=0 rows

[2026-10-06 17:15:28.291 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=15ms p95=17ms max=94ms ≈66fps

[2026-10-06 17:15:28.381 Uid(value=10327):27732:27732 W/bash]
type=1400 audit(0.0:2091350): avc: granted { execute } for name="hexdump" dev="dm-38" ino=8590155 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:28.385 Uid(value=10327):27732:27732 W/bash]
type=1400 audit(0.0:2091351): avc: granted { execute } for name="unshare" dev="dm-38" ino=8590119 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:28.385 Uid(value=10327):27732:27732 W/bash]
type=1400 audit(0.0:2091352): avc: granted { execute } for name="nix" dev="dm-38" ino=8587204 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:28.385 Uid(value=10327):27732:27732 W/bash]
type=1400 audit(0.0:2091353): avc: granted { execute } for name="tabs" dev="dm-38" ino=6594478 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:28.389 Uid(value=10327):27732:27732 W/bash]
type=1400 audit(0.0:2091354): avc: granted { execute } for name="coreutils" dev="dm-38" ino=6585437 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:28.389 Uid(value=10327):27732:27732 W/bash]
type=1400 audit(0.0:2091355): avc: granted { execute } for name="coreutils" dev="dm-38" ino=6585437 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:28.393 Uid(value=10327):27732:27732 W/bash]
type=1400 audit(0.0:2091356): avc: granted { execute } for name="lsblk" dev="dm-38" ino=8590120 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux

[2026-10-06 17:15:29.318 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=0ms scrollback=0 rows

[2026-10-06 17:15:29.318 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=17ms p95=17ms max=101ms ≈58fps

[2026-10-06 17:15:29.518 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=34.529653 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:29.569 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=35.771577 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:30.320 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=1ms p95=0ms max=35ms scrollback=0 rows

[2026-10-06 17:15:30.320 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=16ms p95=17ms max=84ms ≈62fps

[2026-10-06 17:15:30.802 Uid(value=10327):27568:27568 W/RemoteInputConnectionImpl]
requestCursorAnchorInfo on inactive InputConnection

[2026-10-06 17:15:30.825 Uid(value=10327):27568:27568 W/RemoteInputConnectionImpl]
requestCursorAnchorInfo on inactive InputConnection

[2026-10-06 17:15:31.224 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=19ms scrollback=0 rows

[2026-10-06 17:15:31.224 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=15ms p95=17ms max=102ms ≈66fps

[2026-10-06 17:15:32.128 Uid(value=10327):27568:27617 I/native::android::ffi]
searchAllInScrollback: query="g" matches=0

[2026-10-06 17:15:32.179 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=0ms scrollback=0 rows

[2026-10-06 17:15:32.179 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=15ms p95=17ms max=72ms ≈66fps

[2026-10-06 17:15:32.230 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=51.412924 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:33.236 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=51ms scrollback=0 rows

[2026-10-06 17:15:33.236 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=17ms p95=17ms max=76ms ≈58fps

[2026-10-06 17:15:33.287 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=50.303731 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:33.939 Uid(value=10327):27568:27617 I/native::android::ffi]
searchAllInScrollback: query="b" matches=1

[2026-10-06 17:15:34.341 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=1ms p95=0ms max=50ms scrollback=0 rows

[2026-10-06 17:15:34.342 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=18ms p95=45ms max=169ms ≈55fps

[2026-10-06 17:15:35.247 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=0ms p95=0ms max=0ms scrollback=0 rows

[2026-10-06 17:15:35.247 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=15ms p95=17ms max=83ms ≈66fps

[2026-10-06 17:15:35.303 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=56.24173 count=1 newOutput=false scrollOffset=0

[2026-10-06 17:15:35.376 Uid(value=10327):27568:27568 W/RemoteInputConnectionImpl]
requestCursorAnchorInfo on inactive InputConnection

[2026-10-06 17:15:36.275 Uid(value=10327):27568:27739 I/Runtime]
session 2 frame timing window (60 frames): avg=1ms p95=2ms max=56ms scrollback=0 rows

[2026-10-06 17:15:36.275 Uid(value=10327):27568:27739 I/Runtime]
session 2 loop timing window (60 frames): avg=17ms p95=20ms max=109ms ≈58fps

[2026-10-06 17:15:36.325 Uid(value=10327):27568:27739 W/Runtime]
SLOW_FRAME session=2 render=50.067384 count=0 newOutput=false scrollOffset=0

[2026-10-06 17:15:36.756 Uid(value=10327):27568:27568 I/native::android::ffi]
setRenderPaused: paused=true

[2026-10-06 17:15:36.767 Uid(value=10327):27568:27568 I/native::android::ffi]
setRenderPaused: paused=true

[2026-10-06 17:15:36.779 Uid(value=10327):27568:27597 I/mali_gralloc]
unregister: id=0x2990002189d, base=0x0, importpid=27568

[2026-10-06 17:15:36.780 Uid(value=10327):27568:27597 I/mali_gralloc]
unregister: id=0x2990002189e, base=0x0, importpid=27568

[2026-10-06 17:15:36.788 Uid(value=10327):27568:27568 I/mali_gralloc]
unregister: id=0x299000218a7, base=0x0, importpid=27568

