# performance-measurement Specification

## ADDED Requirements

### Requirement: 性能门禁 MUST 测生产路径

实例构建的基准与分配预算 MUST 走生产路径的入口：跨帧复用的实例缓存 + 与渲染线程
相同的调用方式。MUST NOT 另立一条只在用例里存在、而生产不走的构建路径来当门禁——那样
测出的分配与耗时都不是生产实际承受的，门禁形同虚设。

判定口径：基准与预算用例调用的入口 MUST 与渲染线程调用的是同一个。

#### Scenario: 分配预算测的是跨帧复用的缓存

- **WHEN** 检视实例构建的分配预算用例
- **THEN** 它 MUST 在窗口外预热后，跨帧复用同一个实例缓存逐帧重建，
      MUST NOT 每帧新建缓存或经由另一条构建入口

## MODIFIED Requirements

### Requirement: CI 下限断言先预热再采样

CI 性能门禁的下限断言（`native/benches/support.rs` 的 `Floor::assert_within`）MUST 在
正式采样前先执行一次被测操作并**不计入**统计：被测路径的首次调用包含一次性初始化
（字形光栅化与图集上传、查询缓存填充），其成本比稳态高两个数量级，摊进平均会把
「初始化成本」误报成「路径退化」。阈值 MUST 相对**预热后**的稳态设定；不设预热的
断言在慢机上永远误报，等于没有门禁。上限 MUST 按 CI 实测均值校准（每轮采样后打印
`[floor]` 实测行，取其实测的约 1.5 倍），与 `docs/specification/TESTING.md` 的
「基准需要和实际测试结果高度相近以发现之后的性能降低」一致。

#### Scenario: 首调成本不判红

- **WHEN** CI runner 上首次运行 `rebuild_all_rows` 下限断言
- **THEN** 初始化被预热吸收，采样均值反映稳态转换成本，不因首调光栅化失败

#### Scenario: 数量级退化仍然判红

- **WHEN** 转换路径被改成逐格重建字体或 O(n²) 查表
- **THEN** 稳态均值超出上限，断言失败并报出实测均值与采样数

下限断言的阈值 MUST 按**编译档位**缩放：debug 档位不优化，同一操作比 `bench` 档位
（继承 release，含 LTO）慢约一个数量级，用优化档位的阈值去判 debug 档位会把编译档位
差异误报成性能退化。缩放 MUST 只作用于 debug 档位，优化档位（`cargo bench`）的阈值
MUST NOT 缩放——真正的性能门禁在优化档位。

#### Scenario: debug 档位不误报为性能退化

- **WHEN** 在 debug 档位运行 `cargo test --all-targets`（基准目标被一并执行）
- **THEN** 下限断言的阈值按 debug 档位缩放，基准不因档位差异判红

#### Scenario: 优化档位的门禁不被放宽

- **WHEN** 在优化档位运行 `cargo bench`
- **THEN** 下限断言的阈值与 CI 校准值一致，不因档位缩放而放宽

增量路径 MUST 有独立的基准：生产上每来一次输出就走「脏掩码 + 行级实例缓存」这条路，
只测全量构建会漏掉「增量帧退化成整屏遍历」这类固定开销（基准
`rebuild_dirty_one_dirty_row`）。

#### Scenario: 增量帧退化被用例钉住

- **WHEN** 增量帧的预热范围退化成整屏遍历
- **THEN** 用例 `incremental_frame_warms_only_dirty_rows` MUST 因预热访问的单元数超过
      脏行单元数而失败——下限断言留了两个数量级余量，捕不到这种量级的退化
