# render-loop-compile-arena

按职责拆分渲染循环并去掉无挂起点的协程包装，使 ART 编译期的 arena 工作内存降到 4MiB
阈值以下；同时校正「`Compiler allocated` 的数字是编译产物字节数」这一错误口径，并登记
2026-11-20 12-45-04 日志的聚合归因
