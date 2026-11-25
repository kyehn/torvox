use super::*;

fn filter(data: &[u8]) -> (Vec<u8>, Vec<Vec<u8>>) {
    let filtered = filter_input(
        data,
        MediaCapabilities {
            shared_memory: false,
            temp_dir: None,
        },
    );
    (filtered.payload.into_owned(), filtered.responses)
}

/// 过滤一帧并取清屏序列：返回（载荷，清屏列表）。
fn filter_clears(data: &[u8]) -> (Vec<u8>, Vec<Clear>) {
    let filtered = filter_input(
        data,
        MediaCapabilities {
            shared_memory: false,
            temp_dir: None,
        },
    );
    (filtered.payload.into_owned(), filtered.clears)
}

#[test]
fn erase_display_is_reported_with_its_offset() {
    let (payload, clears) = filter_clears(b"text\x1b[H\x1b[2Jmore");
    assert_eq!(payload, b"text\x1b[H\x1b[2Jmore", "清屏序列原样透传");
    assert_eq!(
        clears,
        [Clear {
            offset: 7,
            kind: ClearKind::Display,
        }],
        "ED2 必须被标出，且位置指向序列起点"
    );
}

#[test]
fn erase_scrollback_is_reported() {
    let (_, clears) = filter_clears(b"\x1b[3J");
    assert_eq!(
        clears,
        [Clear {
            offset: 0,
            kind: ClearKind::Scrollback,
        }],
        "ED3 必须被标出"
    );
}

#[test]
fn kitty_scroll_complete_erase_counts_as_erase_display() {
    let (_, clears) = filter_clears(b"\x1b[22J");
    assert_eq!(
        clears,
        [Clear {
            offset: 0,
            kind: ClearKind::Display,
        }],
        "kitty 扩展的 22 与 ED2 同样清活动区"
    );
}

#[test]
fn protected_erase_display_is_reported() {
    let (_, clears) = filter_clears(b"\x1b[?2J");
    assert_eq!(
        clears,
        [Clear {
            offset: 0,
            kind: ClearKind::Display,
        }],
        "受保护的清屏同样清活动区与图像放置"
    );
}

#[test]
fn partial_erase_modes_are_not_reported() {
    // ED0（清到行尾）与 ED1（清到行首）不动整屏，上游也不撤下图像放置。
    let (_, clears) = filter_clears(b"\x1b[0J\x1b[1J");
    assert!(clears.is_empty(), "局部清屏不得触发撤下");
}

#[test]
fn erase_with_space_before_final_is_not_reported() {
    // `CSI 2 SP J` 的中间字节不是私有前缀，上游整条忽略。
    let (_, clears) = filter_clears(b"\x1b[2 J");
    assert!(clears.is_empty(), "上游忽略的清屏不得触发撤下");
}

#[test]
fn decape_reset_is_removed() {
    let (payload, responses) = filter(b"before\x1b[*xafter");
    assert_eq!(payload, b"beforeafter", "DECSACE 复位不得到达上游");
    assert!(responses.is_empty());
}

#[test]
fn decape_region_select_is_removed() {
    let (payload, _) = filter(b"\x1b[2*xcontent");
    assert_eq!(payload, b"content", "DECSACE 区域选择同样未实现，必须丢弃");
}

#[test]
fn other_csi_sequences_are_preserved() {
    let (payload, responses) = filter(b"\x1b[31mred\x1b[0m\x1b[2J\x1b[?25l");
    assert_eq!(payload, b"\x1b[31mred\x1b[0m\x1b[2J\x1b[?25l");
    assert!(responses.is_empty());
}

#[test]
fn csi_with_other_intermediate_is_not_decape() {
    // `CSI 1 $ r` 是 DECCARA（上游同样未实现，但那不是本次处理范围，
    // 不得把它当 DECSACE 误吞——它仍要到达上游）。
    let (payload, _) = filter(b"\x1b[1;2;3;4;5$rX");
    assert_eq!(payload, b"\x1b[1;2;3;4;5$rX");
}

#[test]
fn decape_inside_osc_string_is_preserved() {
    // OSC 字符串内容里含 DECSACE 字节形时不得误伤：字符串整段透传。
    let (payload, _) = filter(b"\x1b]0;title \x1b[*x still title\x07after");
    assert_eq!(payload, b"\x1b]0;title \x1b[*x still title\x07after");
}

#[test]
fn incomplete_csi_at_chunk_end_is_passed_through() {
    let (payload, _) = filter(b"text\x1b[2*");
    assert_eq!(payload, b"text\x1b[2*");
}

/// 超长数字参数不得让整帧解析崩掉：参数位数不受限，裸算在 debug 构建下溢出即
/// panic（debug 构建正是用户安装的形态）。饱和后参数不再是任何清屏参数，
/// 序列原样透传。
#[test]
fn oversized_csi_parameter_does_not_overflow() {
    let (payload, clears) = filter_clears(b"text\x1b[99999999999999999999Jtail");
    assert_eq!(
        payload, b"text\x1b[99999999999999999999Jtail",
        "超长参数既不得崩掉也不得被当成清屏"
    );
    assert!(
        clears.is_empty(),
        "超长参数不匹配任何清屏参数，不得误报清屏"
    );
}

/// 块尾未结束的 APC 不再透传：半条探测漏给上游就是两条 `W/kitty_gfx`。
/// 这里只断言它被扣留且不凭空应答，拼接后的判定见
/// `split_kitty_probe_is_held_until_the_next_chunk`。
#[test]
fn incomplete_kitty_apc_at_chunk_end_is_held() {
    let sequence = b"\x1b_Ga=q,i=3,t=s;AAAA";
    let filtered = filter_input(
        sequence,
        MediaCapabilities {
            shared_memory: false,
            temp_dir: None,
        },
    );
    assert!(filtered.payload.is_empty());
    assert_eq!(filtered.carry.as_ref(), sequence);
    assert!(
        filtered.responses.is_empty(),
        "半条 APC 未到达本层判定，应答不得凭空生成"
    );
}

#[test]
fn direct_medium_probe_is_passed_to_upstream() {
    let (payload, responses) = filter(b"\x1b_Ga=q,i=1,t=d,f=24,s=1,v=1;MTIz\x1b\\");
    assert_eq!(payload, b"\x1b_Ga=q,i=1,t=d,f=24,s=1,v=1;MTIz\x1b\\");
    assert!(responses.is_empty());
}

#[test]
fn shared_memory_probe_is_answered_by_this_layer() {
    let (payload, responses) = filter(b"\x1b_Ga=q,i=3,t=s;AAAA\x1b\\");
    assert!(payload.is_empty(), "本层代答的探测命令不得到达上游");
    assert_eq!(
        responses,
        [b"\x1b_Gi=3;EINVAL: unsupported medium\x1b\\".to_vec()],
        "应答必须与上游格式逐字节一致，否则客户端回退失败"
    );
}

/// kitty 规范里「查询可用传输介质」的示例省略 `a`，而上游按 `t` 处理缺省动作：
/// 这类探测必须与本层已覆盖的显式 `a=t` 同等代答，否则上游会为它记一条
/// `erroneous kitty graphics response`（真机 logcat 的唯一来源）。
#[test]
fn shared_memory_probe_without_action_is_answered_by_this_layer() {
    let (payload, responses) = filter(b"\x1b_Gi=31,s=1,v=1,t=s;AAAA\x1b\\");
    assert!(payload.is_empty(), "本层代答的探测命令不得到达上游");
    assert_eq!(
        responses,
        [b"\x1b_Gi=31;EINVAL: unsupported medium\x1b\\".to_vec()],
        "缺省动作按上游语义就是 transmit，应答必须与显式 a=t 一致"
    );
}

/// 静默等级 ≥2 时不得回包：上游把除 0、1 之外的值一律当「不回包」。
#[test]
fn quiet_level_above_two_suppresses_the_error_response() {
    let (payload, responses) = filter(b"\x1b_Ga=q,i=3,t=s,q=3;AAAA\x1b\\");
    assert!(payload.is_empty(), "本层代答的探测命令不得到达上游");
    assert!(
        responses.is_empty(),
        "q=3 与 q=2 同样静默：上游除 0、1 外一律不回包"
    );
}

/// 缺省动作 + 能服务的介质仍须交回上游：那是真实传输，拦下图像就传不过来。
#[test]
fn shared_memory_probe_without_action_is_passed_when_shared_memory_is_available() {
    let filtered = filter_input(
        b"\x1b_Gi=31,s=1,v=1,t=s;AAAA\x1b\\",
        MediaCapabilities {
            shared_memory: true,
            temp_dir: None,
        },
    );
    assert_eq!(
        filtered.payload.as_ref(),
        b"\x1b_Gi=31,s=1,v=1,t=s;AAAA\x1b\\"
    );
    assert!(filtered.responses.is_empty());
}

#[test]
fn shared_memory_probe_is_answered_when_shared_memory_is_available() {
    let filtered = filter_input(
        b"\x1b_Ga=q,i=3,t=s;AAAA\x1b\\",
        MediaCapabilities {
            shared_memory: true,
            temp_dir: None,
        },
    );
    assert_eq!(filtered.payload.as_ref(), b"\x1b_Ga=q,i=3,t=s;AAAA\x1b\\");
    assert!(filtered.responses.is_empty());
}

/// 文件介质探测同样可能省略 `a`：缺省动作 + 打不开的路径必须由本层代答。
#[test]
fn file_probe_without_action_is_answered_by_this_layer() {
    let (payload, responses) = filter(b"\x1b_Gi=2,t=f;L2RlZmluaXRlbHkvbm90L2hlcmUucG5n\x1b\\");
    assert!(payload.is_empty());
    assert_eq!(
        responses,
        [b"\x1b_Gi=2;EINVAL: invalid data\x1b\\".to_vec()]
    );
}

#[test]
fn file_probe_with_missing_file_is_answered_by_this_layer() {
    // 载荷是 base64 编码的路径("/definitely/not/here.png")。
    let (payload, responses) = filter(b"\x1b_Ga=q,i=2,t=f;L2RlZmluaXRlbHkvbm90L2hlcmUucG5n\x1b\\");
    assert!(payload.is_empty());
    assert_eq!(
        responses,
        [b"\x1b_Gi=2;EINVAL: invalid data\x1b\\".to_vec()]
    );
}

#[test]
fn file_probe_with_existing_file_is_passed_to_upstream() {
    let path = std::env::temp_dir().join("input-filter-probe.txt");
    std::fs::write(&path, b"probe").expect("create probe file");
    let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
    let sequence = format!("\x1b_Ga=q,i=4,t=t;{encoded}\x1b\\");
    let (payload, responses) = filter(sequence.as_bytes());
    let _ = std::fs::remove_file(&path);
    assert_eq!(payload, sequence.as_bytes());
    assert!(responses.is_empty());
}

/// 客户端的能力探测把路径长度当成了数据长度：文件只有 3 字节，上游按声明
/// 长度读必然读不满，先记一条警告再回错误应答。本层按上游的错误语义代答，
/// 终端行为不变而日志不再有探测噪声。
#[test]
fn file_probe_shorter_than_declared_length_is_answered_by_this_layer() {
    let path = std::env::temp_dir().join("input-filter-short-probe.bin");
    std::fs::write(&path, [1u8, 2, 3]).expect("create probe file");
    let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
    let sequence = format!("\x1b_Ga=q,i=5,t=f,S=64;{encoded}\x1b\\");
    let (payload, responses) = filter(sequence.as_bytes());
    let _ = std::fs::remove_file(&path);
    assert!(payload.is_empty(), "本层代答的探测命令不得到达上游");
    assert_eq!(
        responses,
        [b"\x1b_Gi=5;EINVAL: invalid data\x1b\\".to_vec()],
        "读不满时上游回的就是这条错误应答"
    );
}

/// 声明长度与文件等长时必须交回上游：真实传输正是这条路，拦下它图像就传不过来。
#[test]
fn file_probe_matching_the_declared_length_is_passed_to_upstream() {
    let path = std::env::temp_dir().join("input-filter-exact-probe.bin");
    std::fs::write(&path, [1u8, 2, 3, 4, 5]).expect("create probe file");
    let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
    let sequence = format!("\x1b_Ga=q,i=6,t=f,S=5;{encoded}\x1b\\");
    let (payload, responses) = filter(sequence.as_bytes());
    let _ = std::fs::remove_file(&path);
    assert_eq!(payload, sequence.as_bytes());
    assert!(responses.is_empty());
}

/// 偏移也要算进所需长度：上游先跳到偏移再读声明长度，文件不够长就读不满。
#[test]
fn file_probe_with_offset_needs_the_offset_room() {
    let path = std::env::temp_dir().join("input-filter-offset-probe.bin");
    std::fs::write(&path, [1u8, 2, 3, 4, 5, 6]).expect("create probe file");
    let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
    let sequence = format!("\x1b_Ga=q,i=7,t=f,O=3,S=3;{encoded}\x1b\\");
    let (payload, responses) = filter(sequence.as_bytes());
    let _ = std::fs::remove_file(&path);
    assert_eq!(payload, sequence.as_bytes(), "偏移加长度正好等于文件长度");
    assert!(responses.is_empty());
}

/// 偏移加长度超出文件长度时读不满，与没有偏移时同一条错误。
#[test]
fn file_probe_with_offset_past_the_end_is_answered_by_this_layer() {
    let path = std::env::temp_dir().join("input-filter-offset-short-probe.bin");
    std::fs::write(&path, [1u8, 2, 3, 4, 5]).expect("create probe file");
    let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
    let sequence = format!("\x1b_Ga=q,i=8,t=f,O=3,S=3;{encoded}\x1b\\");
    let (payload, responses) = filter(sequence.as_bytes());
    let _ = std::fs::remove_file(&path);
    assert!(payload.is_empty());
    assert_eq!(
        responses,
        [b"\x1b_Gi=8;EINVAL: invalid data\x1b\\".to_vec()]
    );
}

/// 临时文件介质：路径在配置的临时目录里、名字带标记时，本层才敢代答——
/// 上游对临时文件另有两判据，不满足时它给的是更具体的错误。
#[test]
fn temp_file_probe_inside_the_configured_temp_dir_is_answered_by_this_layer() {
    let dir = probe_dir_outside_system_temp();
    let path = dir.path().join("kitty-tty-graphics-protocol-inside");
    std::fs::write(&path, [1u8, 2, 3]).expect("create probe file");
    let encoded = BASE64.encode(path.to_str().expect("probe path is UTF-8"));
    let sequence = format!("\x1b_Ga=q,i=9,t=t,S=64;{encoded}\x1b\\");
    let filtered = filter_input(
        sequence.as_bytes(),
        MediaCapabilities {
            shared_memory: false,
            temp_dir: Some(dir.path()),
        },
    );
    assert!(filtered.payload.is_empty());
    assert_eq!(
        filtered.responses,
        [b"\x1b_Gi=9;EINVAL: invalid data\x1b\\".to_vec()]
    );
}

/// 临时文件介质：路径不在任何临时目录里时交回上游，由上游按自己的判据
/// 给出「not in temp dir」那一条——本层不代答它给不出的错误。
#[test]
fn temp_file_probe_outside_every_temp_dir_is_left_to_upstream() {
    let dir = probe_dir_outside_system_temp();
    let path = dir.path().join("kitty-tty-graphics-protocol-outside");
    std::fs::write(&path, [1u8, 2, 3]).expect("create probe file");
    let encoded = BASE64.encode(path.to_str().expect("probe path is UTF-8"));
    let sequence = format!("\x1b_Ga=q,i=10,t=t,S=64;{encoded}\x1b\\");
    let other_directory = dir.path().join("another-dir");
    let filtered = filter_input(
        sequence.as_bytes(),
        MediaCapabilities {
            shared_memory: false,
            temp_dir: Some(&other_directory),
        },
    );
    assert_eq!(filtered.payload, sequence.as_bytes());
    assert!(filtered.responses.is_empty());
}

/// 临时文件介质：路径是符号链接且指向配置目录之外时，上游按规范化后的路径
/// 判「不在临时目录」，本层不得代答。
#[cfg(unix)]
#[test]
fn temp_file_probe_with_symlink_escaping_the_temp_dir_is_left_to_upstream() {
    let outside = probe_dir_outside_system_temp();
    let temp_dir = probe_dir_outside_system_temp();
    let target = outside.path().join("real-target.bin");
    std::fs::write(&target, [1u8, 2, 3]).expect("create probe file");
    let link = temp_dir.path().join("kitty-tty-graphics-protocol-link");
    std::os::unix::fs::symlink(&target, &link).expect("create symlink");
    let encoded = BASE64.encode(link.to_str().expect("probe path is UTF-8"));
    let sequence = format!("\x1b_Ga=q,i=12,t=t,S=64;{encoded}\x1b\\");
    let filtered = filter_input(
        sequence.as_bytes(),
        MediaCapabilities {
            shared_memory: false,
            temp_dir: Some(temp_dir.path()),
        },
    );
    assert_eq!(filtered.payload, sequence.as_bytes());
    assert!(filtered.responses.is_empty());
}

/// 临时文件介质：符号链接指向的文件名不含标记时，上游按规范化后的路径判
/// 「名字不对」，本层不得代答。
#[cfg(unix)]
#[test]
fn temp_file_probe_with_symlink_renamed_outside_is_left_to_upstream() {
    let dir = probe_dir_outside_system_temp();
    let target = dir.path().join("real-target.bin");
    std::fs::write(&target, [1u8, 2, 3]).expect("create probe file");
    let link = dir.path().join("kitty-tty-graphics-protocol-link");
    std::os::unix::fs::symlink(&target, &link).expect("create symlink");
    let encoded = BASE64.encode(link.to_str().expect("probe path is UTF-8"));
    let sequence = format!("\x1b_Ga=q,i=13,t=t,S=64;{encoded}\x1b\\");
    let filtered = filter_input(
        sequence.as_bytes(),
        MediaCapabilities {
            shared_memory: false,
            temp_dir: Some(dir.path()),
        },
    );
    assert_eq!(filtered.payload, sequence.as_bytes());
    assert!(filtered.responses.is_empty());
}

/// 数据长度是 `S`，`s` 是像素宽：把 `s` 当长度会拦下真实传输（宽度常常大于
/// 文件字节数），也会放过上轮的探测噪声（宽度恒为 1，永远「读得满」）。
#[test]
fn file_probe_pixel_width_is_not_treated_as_the_data_size() {
    let path = std::env::temp_dir().join("input-filter-width-probe.bin");
    std::fs::write(&path, [1u8, 2, 3, 4, 5]).expect("create probe file");
    let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
    let sequence = format!("\x1b_Ga=q,i=11,t=f,s=100;{encoded}\x1b\\");
    let (payload, responses) = filter(sequence.as_bytes());
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        payload,
        sequence.as_bytes(),
        "没有 `S` 时上游把整个文件读完，本层不得拦下"
    );
    assert!(responses.is_empty());
}

/// 探测文件所在的目录：上游的临时目录判据把 `/tmp` 与 `/dev/shm` 无条件算作
/// 临时目录，「文件在临时目录外」的用例必须避开这两处。构建输出目录、当前
/// 目录、用户主目录依次尝试，取第一个规范化后不在临时目录下的可写目录。
fn probe_dir_outside_system_temp() -> tempfile::TempDir {
    let mut bases = vec![std::path::PathBuf::from(env!("OUT_DIR"))];
    if let Ok(current) = std::env::current_dir() {
        bases.push(current);
    }
    if let Some(home) = std::env::var_os("HOME") {
        bases.push(std::path::PathBuf::from(home));
    }
    for base in bases {
        let Ok(canonical) = std::fs::canonicalize(&base) else {
            continue;
        };
        let Some(canonical) = canonical.to_str() else {
            continue;
        };
        if is_path_in_dir("/tmp", canonical) || is_path_in_dir("/dev/shm", canonical) {
            continue;
        }
        if let Ok(dir) = tempfile::Builder::new()
            .prefix("kitty-graphics-probe-")
            .tempdir_in(canonical)
        {
            return dir;
        }
    }
    panic!("no writable directory outside /tmp and /dev/shm for the kitty temp-file probe");
}

#[test]
fn probe_without_image_id_is_dropped_without_response() {
    // 上游对无 id 的查询报 `EINVAL: image ID required` 且压根不回包：
    // 本层同样吞掉命令且不回包，否则客户端会卡在等应答上。
    let (payload, responses) = filter(b"\x1b_Ga=q,t=s;AAAA\x1b\\");
    assert!(payload.is_empty());
    assert!(responses.is_empty());
}

#[test]
fn probe_with_silent_failures_is_dropped_without_response() {
    let (payload, responses) = filter(b"\x1b_Ga=q,i=7,t=s,q=2;AAAA\x1b\\");
    assert!(payload.is_empty());
    assert!(responses.is_empty(), "`q=2` 时上游也不回失败应答");
}

#[test]
fn probe_with_silent_success_still_answers_failures() {
    let (payload, responses) = filter(b"\x1b_Ga=q,i=8,t=s,q=1;AAAA\x1b\\");
    assert!(payload.is_empty());
    assert_eq!(
        responses,
        [b"\x1b_Gi=8;EINVAL: unsupported medium\x1b\\".to_vec()],
        "`q=1` 只静默成功应答，失败仍要回"
    );
}

#[test]
fn transmit_with_missing_file_is_answered_by_this_layer() {
    // 实际传输（非探测）同样可能因介质不可用而失败，须一并代答。
    let (payload, responses) = filter(b"\x1b_Ga=T,i=9,t=f;L2RlZmluaXRlbHkvbm90L2hlcmUucG5n\x1b\\");
    assert!(payload.is_empty());
    assert_eq!(
        responses,
        [b"\x1b_Gi=9;EINVAL: invalid data\x1b\\".to_vec()]
    );
}

#[test]
fn non_query_kitty_commands_are_passed_through() {
    let (payload, responses) = filter(b"\x1b_Ga=p,i=3;OK\x1b\\");
    assert_eq!(payload, b"\x1b_Ga=p,i=3;OK\x1b\\");
    assert!(responses.is_empty());
}

#[test]
fn kitty_apc_followed_by_text_is_split_cleanly() {
    let (payload, responses) = filter(b"\x1b_Ga=q,i=3,t=s;AAAA\x1b\\visible");
    assert_eq!(payload, b"visible");
    assert_eq!(responses.len(), 1);
}

/// 传输动作的失败应答回显图像编号与放置 id（上游的应答格式）。
///
/// 动作必须是传输：查询缺图像 id 时上游压根不编码应答（见
/// [`query_without_image_id_is_dropped_even_with_image_number`]）。
#[test]
fn response_includes_image_number_and_placement_id() {
    let (payload, responses) = filter(b"\x1b_Ga=t,I=13,p=7,t=s;AAAA\x1b\\");
    assert!(payload.is_empty());
    assert_eq!(
        responses,
        [b"\x1b_GI=13,p=7;EINVAL: unsupported medium\x1b\\".to_vec()]
    );
}

/// 探测被 PTY 读边界拆开时，半条 APC 必须扣留到下一块再判定。真机
/// 20:24:16 的两条 `W/kitty_gfx`（`failed to open image file` 与
/// `erroneous kitty graphics response`）正是漏掉的半条探测让上游处理的。
#[test]
fn split_kitty_probe_is_held_until_the_next_chunk() {
    let sequence = b"\x1b_Ga=q,i=2,t=f;L2RlZmluaXRlbHkvbm90L2hlcmUucG5n\x1b\\";
    let split = 12;
    let first = filter_input(
        &sequence[..split],
        MediaCapabilities {
            shared_memory: false,
            temp_dir: None,
        },
    );
    assert!(first.payload.is_empty(), "块尾的半条序列不得透传");
    assert_eq!(first.carry.as_ref(), &sequence[..split]);
    assert!(first.responses.is_empty());

    let mut joined = first.carry.to_vec();
    joined.extend_from_slice(&sequence[split..]);
    let second = filter_input(
        &joined,
        MediaCapabilities {
            shared_memory: false,
            temp_dir: None,
        },
    );
    assert!(second.payload.is_empty());
    assert_eq!(
        second.responses,
        [b"\x1b_Gi=2;EINVAL: invalid data\x1b\\".to_vec()]
    );
    assert!(second.carry.is_empty(), "整条已判定，没有可扣留的尾巴");
}

/// 块尾只剩 `ESC` 或 `ESC _` 时同属「下一块可能正是 `_G` 的后半」，一并扣留；
/// 扣留不影响同一块里之前的普通输出。
#[test]
fn text_before_an_incomplete_apc_is_forwarded_while_the_tail_is_held() {
    let filtered = filter_input(
        b"text\x1b_",
        MediaCapabilities {
            shared_memory: false,
            temp_dir: None,
        },
    );
    assert_eq!(filtered.payload.as_ref(), b"text");
    assert_eq!(filtered.carry.as_ref(), b"\x1b_");
    assert!(filtered.responses.is_empty());
}

/// 超过扣留上限仍未结束的 APC 是 direct 介质的大图：本层从不代答它，
/// 扣留只会把大图推迟到下一块，故原样透传。
#[test]
fn oversized_incomplete_apc_is_forwarded_without_holding() {
    let mut data = b"\x1b_Ga=T,f=100;".to_vec();
    data.extend(std::iter::repeat_n(b'A', KITTY_CARRY_LIMIT));
    let filtered = filter_input(
        &data,
        MediaCapabilities {
            shared_memory: false,
            temp_dir: None,
        },
    );
    assert_eq!(filtered.payload.as_ref(), data.as_slice());
    assert!(filtered.carry.is_empty());
}

#[test]
fn plain_text_without_escape_is_borrowed() {
    let input = b"plain output";
    let filtered = filter_input(
        input,
        MediaCapabilities {
            shared_memory: false,
            temp_dir: None,
        },
    );
    assert!(matches!(filtered.payload, Cow::Borrowed(_)));
    assert!(filtered.responses.is_empty());
}

#[test]
fn control_data_with_unknown_keys_is_ignored() {
    let (payload, responses) = filter(b"\x1b_Ga=q,e=T,i=5,t=s,U=1,C=1;AAAA\x1b\\");
    assert!(payload.is_empty());
    assert_eq!(
        responses,
        [b"\x1b_Gi=5;EINVAL: unsupported medium\x1b\\".to_vec()]
    );
}

/// 上游在键状态里把 `,` 当普通字节收下，于是 `bogus,t` 是一个多字节键：上游转入
/// 忽略态，**此后的所有键值对**都被忽略（不只是那一对），图像 id 因此缺省，
/// 查询没有可编码的应答——上游与本层都不回包。
#[test]
fn multi_byte_key_makes_the_rest_of_the_control_data_ignored() {
    let (payload, responses) = filter(b"\x1b_Ga=q,bogus,t=s,i=6;AAAA\x1b\\");
    assert!(payload.is_empty());
    assert!(responses.is_empty());
}

/// 值超过 11 字节时上游的值缓冲溢出，整对被忽略，且此后的键值对同样被忽略：
/// 图像 id 因此缺省，查询没有可编码的应答——上游与本层都不回包。
#[test]
fn oversized_control_value_makes_the_rest_ignored() {
    let (payload, responses) = filter(b"\x1b_Ga=q,i=123456789012,t=s;AAAA\x1b\\");
    assert!(payload.is_empty());
    assert!(responses.is_empty());
}

/// 载荷不是合法 base64 时上游在解析阶段就判整条命令无效：既不执行也不回应答。
/// 本层同样丢弃，且不留上游那两条协议错误日志。
#[test]
fn invalid_base64_payload_is_dropped_without_response() {
    let (payload, responses) = filter(b"\x1b_Ga=q,i=5,t=f;/ZGVmaW5pdGVseS9ub3QvaGVyZS5wbmc=\x1b\\");
    assert!(payload.is_empty());
    assert!(responses.is_empty());
}

/// 查询缺图像 id 时上游先报 id 缺失（与介质无关），但那条错误的应答没有图像 id
/// 与图像编号，上游压根不编码它——客户端收不到任何字节。
#[test]
fn query_without_image_id_is_dropped_even_with_image_number() {
    let (payload, responses) = filter(b"\x1b_Ga=q,I=13,p=7,t=s;AAAA\x1b\\");
    assert!(payload.is_empty());
    assert!(responses.is_empty());
}

/// 图像 id 与图像编号同时给出时上游在动作分流之前先判互斥：回互斥错误，
/// 根本走不到介质判据。
#[test]
fn image_id_and_number_are_mutually_exclusive() {
    let (payload, responses) = filter(b"\x1b_Ga=q,i=1,I=2,t=s;AAAA\x1b\\");
    assert!(payload.is_empty());
    assert_eq!(
        responses,
        [b"\x1b_Gi=1,I=2;EINVAL: image ID and number are mutually exclusive\x1b\\".to_vec()]
    );
}

/// 单字节非数字值取其 ASCII 码（上游 `finishValue` 的口径）。
#[test]
fn single_character_control_value_is_read_as_its_ascii_code() {
    let (payload, responses) = filter(b"\x1b_Ga=q,i=a,t=s;AAAA\x1b\\");
    assert!(payload.is_empty());
    assert_eq!(
        responses,
        [b"\x1b_Gi=97;EINVAL: unsupported medium\x1b\\".to_vec()]
    );
}

/// 直接用上游终端跑一串 Kitty APC，收走上游给客户端的应答字节。
///
/// 必须绕开本仓的 `GhosttyTerminal`：它内部就跑着本层的输入过滤，拿它的应答
/// 当「上游应答」等于自己跟自己比——那种比法永远成立，查不出任何偏差。
fn upstream_responses(sequence: &[u8]) -> Vec<Vec<u8>> {
    let mut terminal = libghostty_vt::Terminal::new(20, 5).expect("上游终端");
    terminal
        .set_kitty_image_storage_limit(Some(1024 * 1024))
        .expect("kitty 存储上限");
    terminal
        .set_kitty_image_from_file_allowed(true)
        .expect("文件介质");
    terminal
        .set_kitty_image_from_shared_mem_allowed(true)
        .expect("共享内存介质");
    let responses = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
    let sink = std::sync::Arc::clone(&responses);
    terminal
        .on_pty_write(move |_terminal, data| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(data.to_vec());
            }
        })
        .expect("回写回调");
    terminal.vt_write(sequence);
    let guard = responses.lock().expect("应答缓冲");
    guard.clone()
}

/// 代答层与上游必须给出同一条应答：同一串控制数据分别喂给上游终端与本层，
/// 两边给客户端的字节必须一致——层代答时是层给的应答，层放行时是上游给的，
/// 层丢弃命令时上游也必须不回应答。
fn assert_matches_upstream(sequence: &[u8]) {
    let upstream = upstream_responses(sequence);
    let filtered = filter_input(sequence, MediaCapabilities::platform(None));
    if filtered.payload.as_ref() == sequence {
        // 放行：客户端收到的是上游的应答，本层不得另有代答。
        assert!(filtered.responses.is_empty(), "放行的命令不得另有代答");
        return;
    }
    assert_eq!(filtered.responses, upstream, "代答必须与上游逐字节一致");
}

/// 代答层与上游的应答差分：覆盖控制数据解析的各处口径（十进制值与单字节非
/// 数字值的 ASCII 规则、多字节键与超长值让上游忽略其余键值对、值非法即整条
/// 命令无效、id 与编号互斥、查询缺图像 id、载荷不是合法 base64）。
#[test]
fn answers_are_identical_to_upstream() {
    // 打不开的文件路径（base64）。
    const MISSING: &str = "L2RlZmluaXRlbHkvbm90L2hlcmUucG5n";
    // 多了一个填充位：上游解不开，整条命令无效。
    const BAD_BASE64: &str = "/ZGVmaW5pdGVseS9ub3QvaGVyZS5wbmc=";
    let cases = [
        // 图像 id 按十进制解析。
        "\x1b_Ga=q,i=5,t=f;",
        // 单字节非数字值取 ASCII 码。
        "\x1b_Ga=q,i=a,t=f;",
        // 多字节键让上游忽略此后的键值对。
        "\x1b_Ga=q,xx=1,i=5,t=f;",
        "\x1b_Ga=q,bogus,t=f,i=5;",
        // 超长值同样让上游忽略此后的键值对。
        "\x1b_Ga=q,i=123456789012,t=f;",
        // id 与编号互斥先于介质判据。
        "\x1b_Ga=q,i=1,I=2,t=f;",
        // 查询缺图像 id 时没有可编码的应答。
        "\x1b_Ga=q,I=13,p=7,t=f;",
        "\x1b_Ga=q,t=f;",
        // 值不是合法整数即整条命令无效。
        "\x1b_Ga=q,i=abc,t=f;",
        // 空值同样让整条命令无效。
        "\x1b_Ga=q,i=,t=f;",
        // 传输动作回显编号与放置 id。
        "\x1b_Ga=t,I=13,t=f;",
        // 传输并显示同样按介质判据失败。
        "\x1b_Ga=T,i=9,t=f;",
        // 放置 id 与文件内偏移都要回显或参与判据。
        "\x1b_Ga=q,i=5,p=3,t=f;",
        "\x1b_Ga=q,i=5,O=10,t=f;",
        // 静默等级：q=1 只静默成功，q=2 连失败也不回。
        "\x1b_Ga=q,i=5,q=1,t=f;",
        "\x1b_Ga=q,i=5,q=2,t=f;",
        // 声明长度超出文件实际长度。
        "\x1b_Ga=q,i=5,t=f,S=64;",
    ];
    for prefix in cases {
        let sequence = format!("{prefix}{MISSING}\x1b\\");
        assert_matches_upstream(sequence.as_bytes());
    }
    // 载荷不是合法 base64：上游整条丢弃且不回应答。
    let sequence = format!("\x1b_Ga=q,i=5,t=f;{BAD_BASE64}\x1b\\");
    assert_matches_upstream(sequence.as_bytes());
}
