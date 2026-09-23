//! 子进程环境变量白名单步骤：驱动 [`build_env`] 并断言规范变量。

use cucumber::{given, then, when};
use native::terminal::pty::build_env;
use native::terminal::shell_env::ShellEnv;

use super::TerminalWorld;

#[given(expr = "家目录为 {string}")]
pub async fn set_home(world: &mut TerminalWorld, home: String) {
    world.home = home;
}

#[given(expr = "前缀为 {string}")]
pub async fn set_prefix(world: &mut TerminalWorld, prefix: String) {
    world.prefix = Some(prefix);
}

#[given(expr = "mkshrc 路径为 {string}")]
pub async fn set_mkshrc_path(world: &mut TerminalWorld, mkshrc_path: String) {
    world.mkshrc_path = Some(mkshrc_path);
}

#[when("构建子进程环境变量")]
pub async fn build(world: &mut TerminalWorld) {
    world.env = build_env(&ShellEnv {
        home: world.home.clone(),
        working_directory: world.home.clone(),
        prefix: world.prefix.clone(),
        mkshrc_path: world.mkshrc_path.clone(),
    });
}

#[then(expr = "环境变量 {string} 的值为 {string}")]
pub async fn env_eq(world: &mut TerminalWorld, key: String, value: String) {
    assert!(
        world
            .env
            .iter()
            .any(|(name, val)| name == &key && val == &value),
        "环境变量 {key} 缺失或值不符：{val:?}，实际={env:?}",
        val = value,
        env = world.env,
    );
}

#[then(expr = "环境变量中不存在 {string}")]
pub async fn env_absent(world: &mut TerminalWorld, key: String) {
    assert!(
        world.env.iter().all(|(name, _)| name != &key),
        "禁止的环境变量 {key} 不应出现：{env:?}",
        env = world.env,
    );
}
