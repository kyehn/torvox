# logcat W/E 完整清单

本文件逐条列出 `logcat_2026-10-22_18-37-49.txt` 中全部 215 条 `W/`、`E/` 日志，
按原始顺序编号；多行消息以 ` / ` 连接。分类与处置见 `logcat-audit.md`。

| # | 时间 | 级别/标签 | 消息 |
| --- | --- | --- | --- |
| 1 | 2026-10-22 18:35:29.929 | W/TerminalSurface | applySurfaceResize: surface not valid yet, deferring |
| 2 | 2026-10-22 18:35:29.970 | W/wgpu_core::instance | Missing downlevel flags: DownlevelFlags(SURFACE_VIEW_FORMATS) / The underlying API or device in use does not support enough features to be a fully compliant implementation of WebGPU. A subset of the features can still be used. If you are running this program on native and not in a browser and wish to limit the features you use to the supported subset, call Adapter::downlevel_properties or Device::downlevel_properties to get a listing of the features the current platform supports. |
| 3 | 2026-10-22 18:35:29.970 | W/wgpu_core::instance | DownlevelCapabilities { / flags: DownlevelFlags( / COMPUTE_SHADERS \| FRAGMENT_WRITABLE_STORAGE \| INDIRECT_EXECUTION \| BASE_VERTEX \| READ_ONLY_DEPTH_STENCIL \| NON_POWER_OF_TWO_MIPMAPPED_TEXTURES \| CUBE_ARRAY_TEXTURES \| COMPARISON_SAMPLERS \| INDEPENDENT_BLEND \| VERTEX_STORAGE \| ANISOTROPIC_FILTERING \| FRAGMENT_STORAGE \| MULTISAMPLED_SHADING \| DEPTH_TEXTURE_AND_BUFFER_COPIES \| WEBGPU_TEXTURE_FORMAT_SUPPORT \| BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED \| UNRESTRICTED_INDEX_BUFFER \| FULL_DRAW_INDEX_UINT32 \| DEPTH_BIAS_CLAMP \| VIEW_FORMATS \| UNRESTRICTED_EXTERNAL_TEXTURE_COPIES \| NONBLOCKING_QUERY_RESOLVE \| SHADER_F16_IN_F32 \| MSL2_1 \| TEXTURE_COMPRESSION, / ), / limits: DownlevelLimits, / shader_model: Sm5, / } |
| 4 | 2026-10-22 18:35:30.129 | W/libc | Access denied finding property "persist.vendor.gpu.fbc" |
| 5 | 2026-10-22 18:35:30.129 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 6 | 2026-10-22 18:35:30.129 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 7 | 2026-10-22 18:35:30.130 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 8 | 2026-10-22 18:35:30.130 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 9 | 2026-10-22 18:35:30.130 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 10 | 2026-10-22 18:35:30.130 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 11 | 2026-10-22 18:35:30.130 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 12 | 2026-10-22 18:35:30.131 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 13 | 2026-10-22 18:35:30.199 | W/Runtime | SLOW_FRAME session=1 render=47.572039 count=1 newOutput=false scrollOffset=0 |
| 14 | 2026-10-22 18:35:30.199 | W/Runtime | SLOW_LOOP session=1 total=47.969885ms gate=0.003231ms pre=0.099039ms render=47.572692ms poll=0.280231ms tail=0.014692ms |
| 15 | 2026-10-22 18:35:30.216 | E/SurfaceSyncer | Failed to find sync for id=0 |
| 16 | 2026-10-22 18:35:30.275 | W/Parcel | Expecting binder but got null! |
| 17 | 2026-10-22 18:35:30.283 | W/libc | Access denied finding property "persist.unipnp.debug" |
| 18 | 2026-10-22 18:35:30.283 | W/libc | Access denied finding property "persist.unipnp.debug" |
| 19 | 2026-10-22 18:35:30.284 | W/ziparchive | Unable to open '/system_ext/framework/unipnp-framework.dm': No such file or directory |
| 20 | 2026-10-22 18:35:30.284 | W/ziparchive | Unable to open '/system_ext/framework/unipnp-framework.dm': No such file or directory |
| 21 | 2026-10-22 18:35:40.323 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 22 | 2026-10-22 18:35:40.323 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 23 | 2026-10-22 18:35:40.324 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 24 | 2026-10-22 18:35:40.324 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 25 | 2026-10-22 18:35:40.324 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 26 | 2026-10-22 18:35:40.324 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 27 | 2026-10-22 18:35:40.324 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 28 | 2026-10-22 18:35:40.324 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 29 | 2026-10-22 18:35:40.332 | E/SurfaceSyncer | Failed to find sync for id=0 |
| 30 | 2026-10-22 18:35:40.332 | E/SurfaceSyncer | Failed to find sync for id=1 |
| 31 | 2026-10-22 18:35:59.300 | E/com.termux | Not starting debugger since process cannot load the jdwp agent. |
| 32 | 2026-10-22 18:35:59.329 | W/ziparchive | Unable to open '/system_ext/framework/unisoc-framework.dm': No such file or directory |
| 33 | 2026-10-22 18:35:59.329 | W/ziparchive | Unable to open '/system_ext/framework/unisoc-framework.dm': No such file or directory |
| 34 | 2026-10-22 18:35:59.337 | W/FeatureOpt | ensureGetRight for getInt, key=zte_foldable,valueType=101,result=0 |
| 35 | 2026-10-22 18:35:59.342 | W/libc | Access denied finding property "vendor.MB.running" |
| 36 | 2026-10-22 18:36:01.521 | W/FeatureOpt | ensureGetRight for getInt, key=zte_foldable,valueType=101,result=0 |
| 37 | 2026-10-22 18:36:01.531 | E/Typeface | updateDefaultTypeface(fontPath: /data/resource-cache/font/default_font_android_id) |
| 38 | 2026-10-22 18:36:01.716 | W/TerminalSurface | applySurfaceResize: surface not valid yet, deferring |
| 39 | 2026-10-22 18:36:01.754 | W/wgpu_core::instance | Missing downlevel flags: DownlevelFlags(SURFACE_VIEW_FORMATS) / The underlying API or device in use does not support enough features to be a fully compliant implementation of WebGPU. A subset of the features can still be used. If you are running this program on native and not in a browser and wish to limit the features you use to the supported subset, call Adapter::downlevel_properties or Device::downlevel_properties to get a listing of the features the current platform supports. |
| 40 | 2026-10-22 18:36:01.754 | W/wgpu_core::instance | DownlevelCapabilities { / flags: DownlevelFlags( / COMPUTE_SHADERS \| FRAGMENT_WRITABLE_STORAGE \| INDIRECT_EXECUTION \| BASE_VERTEX \| READ_ONLY_DEPTH_STENCIL \| NON_POWER_OF_TWO_MIPMAPPED_TEXTURES \| CUBE_ARRAY_TEXTURES \| COMPARISON_SAMPLERS \| INDEPENDENT_BLEND \| VERTEX_STORAGE \| ANISOTROPIC_FILTERING \| FRAGMENT_STORAGE \| MULTISAMPLED_SHADING \| DEPTH_TEXTURE_AND_BUFFER_COPIES \| WEBGPU_TEXTURE_FORMAT_SUPPORT \| BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED \| UNRESTRICTED_INDEX_BUFFER \| FULL_DRAW_INDEX_UINT32 \| DEPTH_BIAS_CLAMP \| VIEW_FORMATS \| UNRESTRICTED_EXTERNAL_TEXTURE_COPIES \| NONBLOCKING_QUERY_RESOLVE \| SHADER_F16_IN_F32 \| MSL2_1 \| TEXTURE_COMPRESSION, / ), / limits: DownlevelLimits, / shader_model: Sm5, / } |
| 41 | 2026-10-22 18:36:01.917 | W/libc | Access denied finding property "persist.vendor.gpu.fbc" |
| 42 | 2026-10-22 18:36:01.917 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 43 | 2026-10-22 18:36:01.917 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 44 | 2026-10-22 18:36:01.918 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 45 | 2026-10-22 18:36:01.918 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 46 | 2026-10-22 18:36:01.918 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 47 | 2026-10-22 18:36:01.918 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 48 | 2026-10-22 18:36:01.918 | E/mali_gralloc | Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator |
| 49 | 2026-10-22 18:36:01.918 | E/mali_gralloc | ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00 |
| 50 | 2026-10-22 18:36:01.989 | W/Runtime | SLOW_FRAME session=1 render=50.152616 count=1 newOutput=false scrollOffset=0 |
| 51 | 2026-10-22 18:36:01.989 | W/Runtime | SLOW_LOOP session=1 total=51.032192ms gate=0.003731ms pre=0.153769ms render=50.154039ms poll=0.698846ms tail=0.021807ms |
| 52 | 2026-10-22 18:36:02.005 | E/SurfaceSyncer | Failed to find sync for id=0 |
| 53 | 2026-10-22 18:36:02.067 | W/Parcel | Expecting binder but got null! |
| 54 | 2026-10-22 18:36:02.090 | W/libc | Access denied finding property "persist.unipnp.debug" |
| 55 | 2026-10-22 18:36:02.090 | W/libc | Access denied finding property "persist.unipnp.debug" |
| 56 | 2026-10-22 18:36:02.090 | W/ziparchive | Unable to open '/system_ext/framework/unipnp-framework.dm': No such file or directory |
| 57 | 2026-10-22 18:36:02.090 | W/ziparchive | Unable to open '/system_ext/framework/unipnp-framework.dm': No such file or directory |
| 58 | 2026-10-22 18:36:06.185 | W/stream | unimplemented CSI action: ESC [ * {  } x |
| 59 | 2026-10-22 18:36:06.204 | W/kitty_gfx | erroneous kitty graphics response: EINVAL: unsupported medium |
| 60 | 2026-10-22 18:36:06.767 | W/Runtime | SLOW_FRAME session=1 render=87.708692 count=1 newOutput=true scrollOffset=0 |
| 61 | 2026-10-22 18:36:06.767 | W/Runtime | SLOW_LOOP session=1 total=102.929539ms gate=14.878808ms pre=0.021ms render=87.709423ms poll=0.310654ms tail=0.009654ms |
| 62 | 2026-10-22 18:36:17.910 | W/fish | type=1400 audit(0.0:10014001): avc: granted { execute } for name="prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 63 | 2026-10-22 18:36:17.910 | W/fish | type=1400 audit(0.0:10014002): avc: granted { execute_no_trans } for path="/data/data/com.termux/files/usr/tmp/prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 64 | 2026-10-22 18:36:17.910 | W/prooted-2448-cB | type=1400 audit(0.0:10014003): avc: granted { execute } for path="/data/data/com.termux/files/usr/tmp/prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 65 | 2026-10-22 18:36:17.910 | W/prooted-2448-cB | type=1400 audit(0.0:10014004): avc: granted { execute } for path="/data/data/com.termux/files/usr/nix/store/6cra80mm42k58z65k59vgikczi0dvvx8-eza-0.23.5/bin/eza" dev="dm-38" ino=7002357 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 66 | 2026-10-22 18:36:17.910 | W/prooted-2448-cB | type=1400 audit(0.0:10014005): avc: granted { execute } for path="/data/data/com.termux/files/usr/nix/store/hx9wvdx2l59iahwf9nmrx7rwkm6cymgs-glibc-2.44-25/lib/ld-linux-aarch64.so.1" dev="dm-38" ino=8604897 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 67 | 2026-10-22 18:36:17.914 | W/eza | type=1400 audit(0.0:10014006): avc: granted { execute } for path="/data/data/com.termux/files/usr/nix/store/r8vv5p4cbak5d2zsvqpmxyy9p2aw3mrc-libgit2-1.9.7-lib/lib/libgit2.so.1.9.7" dev="dm-38" ino=6857609 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 68 | 2026-10-22 18:36:25.377 | W/Runtime | SLOW_FRAME session=1 render=56.763039 count=1 newOutput=true scrollOffset=0 |
| 69 | 2026-10-22 18:36:25.377 | W/Runtime | SLOW_LOOP session=1 total=57.001462ms gate=0.007693ms pre=0.003077ms render=56.763077ms poll=0.206769ms tail=0.020846ms |
| 70 | 2026-10-22 18:36:29.426 | W/fish | type=1400 audit(0.0:10014039): avc: granted { execute } for name="prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 71 | 2026-10-22 18:36:29.426 | W/fish | type=1400 audit(0.0:10014040): avc: granted { execute_no_trans } for path="/data/data/com.termux/files/usr/tmp/prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 72 | 2026-10-22 18:36:29.426 | W/prooted-2448-cB | type=1400 audit(0.0:10014041): avc: granted { execute } for path="/data/data/com.termux/files/usr/tmp/prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 73 | 2026-10-22 18:36:29.515 | W/stream | unimplemented CSI action: ESC [ * {  } x |
| 74 | 2026-10-22 18:36:29.527 | W/kitty_gfx | erroneous kitty graphics response: EINVAL: unsupported medium |
| 75 | 2026-10-22 18:36:29.968 | W/Runtime | SLOW_FRAME session=1 render=65.806154 count=1 newOutput=false scrollOffset=0 |
| 76 | 2026-10-22 18:36:29.968 | W/Runtime | SLOW_LOOP session=1 total=67.762885ms gate=1.447924ms pre=0.00173ms render=65.806385ms poll=0.495615ms tail=0.011231ms |
| 77 | 2026-10-22 18:36:33.838 | W/Runtime | SLOW_FRAME session=1 render=55.574115 count=1 newOutput=true scrollOffset=0 |
| 78 | 2026-10-22 18:36:33.839 | W/Runtime | SLOW_LOOP session=1 total=56.587462ms gate=0.822846ms pre=0.001346ms render=55.574154ms poll=0.182ms tail=0.007116ms |
| 79 | 2026-10-22 18:36:36.126 | W/fish | type=1400 audit(0.0:10014072): avc: granted { execute } for name="prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 80 | 2026-10-22 18:36:36.126 | W/fish | type=1400 audit(0.0:10014073): avc: granted { execute_no_trans } for path="/data/data/com.termux/files/usr/tmp/prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 81 | 2026-10-22 18:36:36.126 | W/prooted-2448-cB | type=1400 audit(0.0:10014074): avc: granted { execute } for path="/data/data/com.termux/files/usr/tmp/prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 82 | 2026-10-22 18:36:36.214 | W/stream | unimplemented CSI action: ESC [ * {  } x |
| 83 | 2026-10-22 18:36:36.224 | W/kitty_gfx | erroneous kitty graphics response: EINVAL: unsupported medium |
| 84 | 2026-10-22 18:36:36.664 | W/Runtime | SLOW_FRAME session=1 render=67.753038 count=1 newOutput=true scrollOffset=0 |
| 85 | 2026-10-22 18:36:36.664 | W/Runtime | SLOW_LOOP session=1 total=76.595346ms gate=8.566731ms pre=0.001577ms render=67.753115ms poll=0.259846ms tail=0.014077ms |
| 86 | 2026-10-22 18:36:38.194 | W/fish | type=1400 audit(0.0:10014088): avc: granted { execute } for name="prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 87 | 2026-10-22 18:36:38.194 | W/fish | type=1400 audit(0.0:10014089): avc: granted { execute_no_trans } for path="/data/data/com.termux/files/usr/tmp/prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 88 | 2026-10-22 18:36:40.166 | W/fish | type=1400 audit(0.0:10014109): avc: granted { execute } for name="prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 89 | 2026-10-22 18:36:40.166 | W/fish | type=1400 audit(0.0:10014110): avc: granted { execute_no_trans } for path="/data/data/com.termux/files/usr/tmp/prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 90 | 2026-10-22 18:36:43.430 | W/fish | type=1400 audit(0.0:10014134): avc: granted { execute } for name="prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 91 | 2026-10-22 18:36:43.430 | W/fish | type=1400 audit(0.0:10014135): avc: granted { execute_no_trans } for path="/data/data/com.termux/files/usr/tmp/prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 92 | 2026-10-22 18:36:43.430 | W/prooted-2448-cB | type=1400 audit(0.0:10014136): avc: granted { execute } for path="/data/data/com.termux/files/usr/tmp/prooted-2448-cBomPc" dev="dm-38" ino=6935339 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 93 | 2026-10-22 18:36:43.430 | W/prooted-2448-cB | type=1400 audit(0.0:10014137): avc: granted { execute } for path="/data/data/com.termux/files/usr/nix/store/dq1apsyiah48nmc4n45dpwc33k887hdn-coreutils-9.11/bin/coreutils" dev="dm-38" ino=6847359 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 94 | 2026-10-22 18:36:46.730 | W/DefaultDispatch | type=1400 audit(0.0:10014143): avc: granted { execute } for name="login" dev="dm-38" ino=7411599 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 95 | 2026-10-22 18:36:46.730 | W/DefaultDispatch | type=1400 audit(0.0:10014144): avc: granted { execute_no_trans } for path="/data/user/0/com.termux/files/usr/bin/login" dev="dm-38" ino=7411599 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 96 | 2026-10-22 18:36:46.742 | W/login | type=1400 audit(0.0:10014145): avc: granted { execute } for path="/data/user/0/com.termux/files/usr/bin/login" dev="dm-38" ino=7411599 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 97 | 2026-10-22 18:36:46.746 | W/login | type=1400 audit(0.0:10014146): avc: denied { getattr } for path="/proc/modules" dev="proc" ino=4026532130 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:proc_modules:s0 tclass=file permissive=0 app=com.termux |
| 98 | 2026-10-22 18:36:46.746 | W/login | type=1400 audit(0.0:10014147): avc: denied { getattr } for path="/proc/bus/pci/devices" dev="proc" ino=4026532146 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:proc:s0 tclass=file permissive=0 app=com.termux |
| 99 | 2026-10-22 18:36:46.773 | W/Runtime | SLOW_FRAME session=1 render=43.270116 count=1 newOutput=false scrollOffset=0 |
| 100 | 2026-10-22 18:36:46.773 | W/Runtime | SLOW_LOOP session=1 total=45.114539ms gate=1.698539ms pre=0.002ms render=43.270154ms poll=0.136077ms tail=0.007769ms |
| 101 | 2026-10-22 18:36:46.852 | W/Runtime | SLOW_FRAME session=1 render=78.929615 count=1 newOutput=false scrollOffset=0 |
| 102 | 2026-10-22 18:36:46.890 | W/Runtime | SLOW_FRAME session=1 render=37.649731 count=1 newOutput=false scrollOffset=0 |
| 103 | 2026-10-22 18:36:46.970 | W/Runtime | SLOW_FRAME session=2 render=30.478538 count=0 newOutput=false scrollOffset=0 |
| 104 | 2026-10-22 18:36:46.970 | W/Runtime | SLOW_LOOP session=2 total=42.771846ms gate=12.127385ms pre=0.007846ms render=30.479192ms poll=0.151385ms tail=0.006038ms |
| 105 | 2026-10-22 18:36:50.370 | W/Runtime | SLOW_FRAME session=2 render=88.784577 count=1 newOutput=false scrollOffset=0 |
| 106 | 2026-10-22 18:36:50.370 | W/Runtime | SLOW_LOOP session=2 total=89.913346ms gate=0.202846ms pre=0.006346ms render=88.785385ms poll=0.88ms tail=0.038769ms |
| 107 | 2026-10-22 18:36:50.456 | W/Runtime | SLOW_FRAME session=3 render=53.279385 count=1 newOutput=false scrollOffset=0 |
| 108 | 2026-10-22 18:36:50.456 | W/Runtime | SLOW_LOOP session=3 total=53.439385ms gate=5.0E-4ms pre=0.037462ms render=53.279731ms poll=0.112884ms tail=0.008808ms |
| 109 | 2026-10-22 18:37:01.215 | E/native::terminal::session | wait thread: child exited with code 254 (possible execve errno=154; see PTY output for the authoritative marker) |
| 110 | 2026-10-22 18:37:01.985 | W/native::terminal::session | session exit: 进程组不可用，仅直杀子进程 2795 |
| 111 | 2026-10-22 18:37:02.035 | W/native::terminal::session | session exit: 进程组不可用，仅直杀子进程 2795 |
| 112 | 2026-10-22 18:37:03.785 | W/Runtime | SLOW_FRAME session=1 render=60.918577 count=1 newOutput=false scrollOffset=0 |
| 113 | 2026-10-22 18:37:03.785 | W/Runtime | SLOW_LOOP session=1 total=61.063462ms gate=3.85E-4ms pre=0.022577ms render=60.919115ms poll=0.112577ms tail=0.008808ms |
| 114 | 2026-10-22 18:37:04.556 | W/Runtime | SLOW_FRAME session=1 render=56.603153 count=1 newOutput=false scrollOffset=1 |
| 115 | 2026-10-22 18:37:04.556 | W/Runtime | SLOW_LOOP session=1 total=56.735539ms gate=3.08E-4ms pre=0.004077ms render=56.603384ms poll=0.118808ms tail=0.008962ms |
| 116 | 2026-10-22 18:37:04.618 | W/Runtime | SLOW_FRAME session=1 render=59.185039 count=1 newOutput=false scrollOffset=2 |
| 117 | 2026-10-22 18:37:04.678 | W/Runtime | SLOW_FRAME session=1 render=58.348423 count=1 newOutput=false scrollOffset=5 |
| 118 | 2026-10-22 18:37:04.825 | W/Runtime | SLOW_FRAME session=1 render=58.492115 count=1 newOutput=false scrollOffset=6 |
| 119 | 2026-10-22 18:37:05.028 | W/Runtime | SLOW_FRAME session=1 render=59.260269 count=1 newOutput=false scrollOffset=7 |
| 120 | 2026-10-22 18:37:05.095 | W/Runtime | SLOW_FRAME session=1 render=60.264577 count=1 newOutput=false scrollOffset=8 |
| 121 | 2026-10-22 18:37:05.095 | W/Runtime | SLOW_LOOP session=1 total=60.416154ms gate=5.39E-4ms pre=0.003769ms render=60.264808ms poll=0.141846ms tail=0.005192ms |
| 122 | 2026-10-22 18:37:06.365 | W/Runtime | SLOW_FRAME session=1 render=57.595577 count=1 newOutput=false scrollOffset=9 |
| 123 | 2026-10-22 18:37:06.365 | W/Runtime | SLOW_LOOP session=1 total=57.945307ms gate=8.07E-4ms pre=0.005808ms render=57.595808ms poll=0.338038ms tail=0.004846ms |
| 124 | 2026-10-22 18:37:06.427 | W/Runtime | SLOW_FRAME session=1 render=60.110193 count=1 newOutput=false scrollOffset=12 |
| 125 | 2026-10-22 18:37:06.489 | W/Runtime | SLOW_FRAME session=1 render=59.727269 count=1 newOutput=false scrollOffset=15 |
| 126 | 2026-10-22 18:37:06.552 | W/Runtime | SLOW_FRAME session=1 render=60.081154 count=1 newOutput=false scrollOffset=17 |
| 127 | 2026-10-22 18:37:06.815 | W/Runtime | SLOW_FRAME session=1 render=58.937693 count=1 newOutput=false scrollOffset=18 |
| 128 | 2026-10-22 18:37:06.877 | W/Runtime | SLOW_FRAME session=1 render=59.929308 count=1 newOutput=false scrollOffset=21 |
| 129 | 2026-10-22 18:37:06.877 | W/Runtime | SLOW_LOOP session=1 total=60.085539ms gate=0.020924ms pre=0.004807ms render=59.929462ms poll=0.126269ms tail=0.004077ms |
| 130 | 2026-10-22 18:37:06.939 | W/Runtime | SLOW_FRAME session=1 render=59.65627 count=1 newOutput=false scrollOffset=24 |
| 131 | 2026-10-22 18:37:07.001 | W/Runtime | SLOW_FRAME session=1 render=59.66023 count=1 newOutput=false scrollOffset=25 |
| 132 | 2026-10-22 18:37:07.067 | W/Runtime | SLOW_FRAME session=1 render=62.726462 count=1 newOutput=false scrollOffset=27 |
| 133 | 2026-10-22 18:37:07.130 | W/Runtime | SLOW_FRAME session=1 render=62.533654 count=1 newOutput=false scrollOffset=28 |
| 134 | 2026-10-22 18:37:07.248 | W/Runtime | SLOW_FRAME session=1 render=58.746923 count=1 newOutput=false scrollOffset=29 |
| 135 | 2026-10-22 18:37:07.310 | W/Runtime | SLOW_FRAME session=1 render=59.773923 count=1 newOutput=false scrollOffset=34 |
| 136 | 2026-10-22 18:37:07.374 | W/Runtime | SLOW_FRAME session=1 render=61.758346 count=1 newOutput=false scrollOffset=35 |
| 137 | 2026-10-22 18:37:08.084 | W/Runtime | SLOW_FRAME session=1 render=60.311692 count=1 newOutput=false scrollOffset=32 |
| 138 | 2026-10-22 18:37:08.085 | W/Runtime | SLOW_LOOP session=1 total=60.459615ms gate=0.001692ms pre=0.006039ms render=60.312384ms poll=0.130231ms tail=0.009269ms |
| 139 | 2026-10-22 18:37:08.148 | W/Runtime | SLOW_FRAME session=1 render=60.136038 count=1 newOutput=false scrollOffset=22 |
| 140 | 2026-10-22 18:37:08.211 | W/Runtime | SLOW_FRAME session=1 render=59.871269 count=1 newOutput=false scrollOffset=7 |
| 141 | 2026-10-22 18:37:08.272 | W/Runtime | SLOW_FRAME session=1 render=59.299693 count=1 newOutput=false scrollOffset=1 |
| 142 | 2026-10-22 18:37:08.484 | W/Runtime | SLOW_FRAME session=1 render=55.758 count=1 newOutput=false scrollOffset=0 |
| 143 | 2026-10-22 18:37:12.298 | W/native::terminal::session | session exit: 进程组不可用，仅直杀子进程 2448 |
| 144 | 2026-10-22 18:37:12.348 | W/native::terminal::session | session exit: 进程组不可用，仅直杀子进程 2448 |
| 145 | 2026-10-22 18:37:13.835 | W/Parcel | Expecting binder but got null! |
| 146 | 2026-10-22 18:37:13.987 | W/Parcel | Expecting binder but got null! |
| 147 | 2026-10-22 18:37:13.998 | W/Parcel | Expecting binder but got null! |
| 148 | 2026-10-22 18:37:15.114 | W/InputEventReceiver | Attempted to finish an input event but the input event receiver has already been disposed. |
| 149 | 2026-10-22 18:37:15.129 | W/Parcel | Expecting binder but got null! |
| 150 | 2026-10-22 18:37:15.140 | W/Parcel | Expecting binder but got null! |
| 151 | 2026-10-22 18:37:15.165 | W/Parcel | Expecting binder but got null! |
| 152 | 2026-10-22 18:37:15.540 | W/Parcel | Expecting binder but got null! |
| 153 | 2026-10-22 18:37:16.253 | W/InputEventReceiver | Attempted to finish an input event but the input event receiver has already been disposed. |
| 154 | 2026-10-22 18:37:25.998 | W/fish | type=1400 audit(0.0:10014227): avc: granted { execute } for name="comma" dev="dm-38" ino=7361440 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 155 | 2026-10-22 18:37:25.998 | W/fish | type=1400 audit(0.0:10014228): avc: granted { execute } for name="comma" dev="dm-38" ino=7361440 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 156 | 2026-10-22 18:37:26.602 | W/fish | type=1400 audit(0.0:10014229): avc: granted { execute } for name="comma" dev="dm-38" ino=7361440 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 157 | 2026-10-22 18:37:26.606 | W/fish | type=1400 audit(0.0:10014230): avc: granted { execute } for name="comma" dev="dm-38" ino=7361440 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 158 | 2026-10-22 18:37:27.002 | W/fish | type=1400 audit(0.0:10014231): avc: granted { execute } for name="comma" dev="dm-38" ino=7361440 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 159 | 2026-10-22 18:37:27.006 | W/proot | type=1400 audit(0.0:10014232): avc: granted { execute } for name="comma" dev="dm-38" ino=7361440 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 160 | 2026-10-22 18:37:27.006 | W/proot | type=1400 audit(0.0:10014233): avc: granted { execute } for name="ld-linux-aarch64.so.1" dev="dm-38" ino=8604897 scontext=u:r:untrusted_app_27:s0:c71,c257,c512,c768 tcontext=u:object_r:app_data_file:s0:c71,c257,c512,c768 tclass=file app=com.termux |
| 161 | 2026-10-22 18:37:27.108 | W/stream | unimplemented CSI action: ESC [ * {  } x |
| 162 | 2026-10-22 18:37:27.118 | W/kitty_gfx | erroneous kitty graphics response: EINVAL: unsupported medium |
| 163 | 2026-10-22 18:37:27.622 | W/Runtime | SLOW_FRAME session=2 render=64.269307 count=1 newOutput=true scrollOffset=0 |
| 164 | 2026-10-22 18:37:27.622 | W/Runtime | SLOW_LOOP session=2 total=72.411231ms gate=7.959731ms pre=0.004461ms render=64.269423ms poll=0.167769ms tail=0.009847ms |
| 165 | 2026-10-22 18:37:28.532 | W/Runtime | SLOW_FRAME session=2 render=60.533192 count=1 newOutput=false scrollOffset=1 |
| 166 | 2026-10-22 18:37:28.532 | W/Runtime | SLOW_LOOP session=2 total=60.664115ms gate=8.07E-4ms pre=0.008154ms render=60.533846ms poll=0.115769ms tail=0.005539ms |
| 167 | 2026-10-22 18:37:28.595 | W/Runtime | SLOW_FRAME session=2 render=60.869923 count=1 newOutput=false scrollOffset=4 |
| 168 | 2026-10-22 18:37:28.658 | W/Runtime | SLOW_FRAME session=2 render=60.881961 count=1 newOutput=false scrollOffset=7 |
| 169 | 2026-10-22 18:37:28.722 | W/Runtime | SLOW_FRAME session=2 render=61.093923 count=1 newOutput=false scrollOffset=9 |
| 170 | 2026-10-22 18:37:28.784 | W/Runtime | SLOW_FRAME session=2 render=59.6895 count=1 newOutput=false scrollOffset=11 |
| 171 | 2026-10-22 18:37:28.851 | W/Runtime | SLOW_FRAME session=2 render=63.684653 count=1 newOutput=false scrollOffset=14 |
| 172 | 2026-10-22 18:37:28.915 | W/Runtime | SLOW_FRAME session=2 render=64.011269 count=1 newOutput=false scrollOffset=15 |
| 173 | 2026-10-22 18:37:28.999 | W/Runtime | SLOW_FRAME session=2 render=59.607693 count=1 newOutput=false scrollOffset=16 |
| 174 | 2026-10-22 18:37:29.061 | W/Runtime | SLOW_FRAME session=2 render=60.077154 count=1 newOutput=false scrollOffset=18 |
| 175 | 2026-10-22 18:37:29.061 | W/Runtime | SLOW_LOOP session=2 total=60.206308ms gate=0.010692ms pre=0.005308ms render=60.077423ms poll=0.107654ms tail=0.005231ms |
| 176 | 2026-10-22 18:37:29.124 | W/Runtime | SLOW_FRAME session=2 render=61.400731 count=1 newOutput=false scrollOffset=19 |
| 177 | 2026-10-22 18:37:30.153 | W/Runtime | SLOW_FRAME session=2 render=58.537461 count=1 newOutput=false scrollOffset=17 |
| 178 | 2026-10-22 18:37:30.153 | W/Runtime | SLOW_LOOP session=2 total=58.680923ms gate=6.53E-4ms pre=0.004385ms render=58.537615ms poll=0.13227ms tail=0.006ms |
| 179 | 2026-10-22 18:37:30.217 | W/Runtime | SLOW_FRAME session=2 render=61.329231 count=1 newOutput=false scrollOffset=14 |
| 180 | 2026-10-22 18:37:30.277 | W/Runtime | SLOW_FRAME session=2 render=58.737308 count=1 newOutput=false scrollOffset=11 |
| 181 | 2026-10-22 18:37:30.341 | W/Runtime | SLOW_FRAME session=2 render=60.867577 count=1 newOutput=false scrollOffset=9 |
| 182 | 2026-10-22 18:37:30.405 | W/Runtime | SLOW_FRAME session=2 render=61.996539 count=1 newOutput=false scrollOffset=7 |
| 183 | 2026-10-22 18:37:30.468 | W/Runtime | SLOW_FRAME session=2 render=59.680423 count=1 newOutput=false scrollOffset=6 |
| 184 | 2026-10-22 18:37:30.533 | W/Runtime | SLOW_FRAME session=2 render=62.743577 count=1 newOutput=false scrollOffset=5 |
| 185 | 2026-10-22 18:37:30.597 | W/Runtime | SLOW_FRAME session=2 render=60.937423 count=1 newOutput=false scrollOffset=4 |
| 186 | 2026-10-22 18:37:30.661 | W/Runtime | SLOW_FRAME session=2 render=60.777693 count=1 newOutput=false scrollOffset=3 |
| 187 | 2026-10-22 18:37:30.661 | W/Runtime | SLOW_LOOP session=2 total=60.975115ms gate=0.074654ms pre=0.006423ms render=60.777885ms poll=0.110577ms tail=0.005576ms |
| 188 | 2026-10-22 18:37:30.724 | W/Runtime | SLOW_FRAME session=2 render=59.350654 count=1 newOutput=false scrollOffset=0 |
| 189 | 2026-10-22 18:37:31.222 | W/Runtime | SLOW_FRAME session=2 render=59.362961 count=1 newOutput=false scrollOffset=1 |
| 190 | 2026-10-22 18:37:31.222 | W/Runtime | SLOW_LOOP session=2 total=59.4975ms gate=0.001308ms pre=0.010192ms render=59.363346ms poll=0.117039ms tail=0.005615ms |
| 191 | 2026-10-22 18:37:31.289 | W/Runtime | SLOW_FRAME session=2 render=62.934115 count=1 newOutput=false scrollOffset=6 |
| 192 | 2026-10-22 18:37:31.352 | W/Runtime | SLOW_FRAME session=2 render=63.4245 count=1 newOutput=false scrollOffset=9 |
| 193 | 2026-10-22 18:37:31.416 | W/Runtime | SLOW_FRAME session=2 render=63.145077 count=1 newOutput=false scrollOffset=14 |
| 194 | 2026-10-22 18:37:31.480 | W/Runtime | SLOW_FRAME session=2 render=64.558731 count=1 newOutput=false scrollOffset=18 |
| 195 | 2026-10-22 18:37:31.639 | W/Runtime | SLOW_FRAME session=2 render=59.424039 count=1 newOutput=false scrollOffset=19 |
| 196 | 2026-10-22 18:37:32.210 | W/Runtime | SLOW_FRAME session=2 render=57.356538 count=1 newOutput=false scrollOffset=18 |
| 197 | 2026-10-22 18:37:32.210 | W/Runtime | SLOW_LOOP session=2 total=57.557731ms gate=0.012769ms pre=0.005039ms render=57.356807ms poll=0.178308ms tail=0.004808ms |
| 198 | 2026-10-22 18:37:32.276 | W/Runtime | SLOW_FRAME session=2 render=63.623116 count=1 newOutput=false scrollOffset=8 |
| 199 | 2026-10-22 18:37:32.340 | W/Runtime | SLOW_FRAME session=2 render=64.198346 count=1 newOutput=false scrollOffset=1 |
| 200 | 2026-10-22 18:37:32.641 | W/Runtime | SLOW_FRAME session=2 render=60.694846 count=1 newOutput=false scrollOffset=0 |
| 201 | 2026-10-22 18:37:33.274 | W/Runtime | SLOW_FRAME session=2 render=57.747808 count=1 newOutput=false scrollOffset=2 |
| 202 | 2026-10-22 18:37:33.274 | W/Runtime | SLOW_LOOP session=2 total=57.873269ms gate=3.46E-4ms pre=0.003346ms render=57.747962ms poll=0.116077ms tail=0.005538ms |
| 203 | 2026-10-22 18:37:33.337 | W/Runtime | SLOW_FRAME session=2 render=60.953962 count=1 newOutput=false scrollOffset=6 |
| 204 | 2026-10-22 18:37:33.402 | W/Runtime | SLOW_FRAME session=2 render=63.109039 count=1 newOutput=false scrollOffset=9 |
| 205 | 2026-10-22 18:37:33.465 | W/Runtime | SLOW_FRAME session=2 render=59.788769 count=1 newOutput=false scrollOffset=13 |
| 206 | 2026-10-22 18:37:33.528 | W/Runtime | SLOW_FRAME session=2 render=60.05727 count=1 newOutput=false scrollOffset=16 |
| 207 | 2026-10-22 18:37:33.592 | W/Runtime | SLOW_FRAME session=2 render=62.286961 count=1 newOutput=false scrollOffset=18 |
| 208 | 2026-10-22 18:37:33.676 | W/Runtime | SLOW_FRAME session=2 render=59.535462 count=1 newOutput=false scrollOffset=19 |
| 209 | 2026-10-22 18:37:34.145 | W/Runtime | SLOW_FRAME session=2 render=61.063692 count=1 newOutput=false scrollOffset=18 |
| 210 | 2026-10-22 18:37:34.145 | W/Runtime | SLOW_LOOP session=2 total=61.170615ms gate=7.3E-4ms pre=0.004654ms render=61.064ms poll=0.096539ms tail=0.004692ms |
| 211 | 2026-10-22 18:37:34.207 | W/Runtime | SLOW_FRAME session=2 render=60.139154 count=1 newOutput=false scrollOffset=10 |
| 212 | 2026-10-22 18:37:34.271 | W/Runtime | SLOW_FRAME session=2 render=61.97577 count=1 newOutput=false scrollOffset=1 |
| 213 | 2026-10-22 18:37:34.561 | W/Runtime | SLOW_FRAME session=2 render=59.804039 count=1 newOutput=false scrollOffset=0 |
| 214 | 2026-10-22 18:37:43.236 | W/native::terminal::session | session exit: 进程组不可用，仅直杀子进程 2755 |
| 215 | 2026-10-22 18:37:43.287 | W/native::terminal::session | session exit: 进程组不可用，仅直杀子进程 2755 |
