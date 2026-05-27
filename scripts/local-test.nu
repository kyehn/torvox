# Torvox 快速本地测试 (nushell)
# 使用: nu scripts/local-test.nu

let results_dir = "test-results"
mkdir $results_dir

print "=== Torvox 本地测试 ==="
print ""

# 1. Clippy
print "[1/5] cargo clippy"
cargo clippy --workspace -- -D warnings o> $"($results_dir)/clippy.log"
print "clippy 零警告"

# 2. Fmt
print "[2/5] cargo fmt"
cargo fmt --check o> $"($results_dir)/fmt.log"
print "格式化通过"

# 3. Rust 测试
print "[3/5] cargo test"
cargo test --workspace
print "Rust 测试通过"

# 4. Android lint
print "[4/5] Android lint"
cd android
./gradlew lint --quiet o> $"../($results_dir)/android-lint.log"
cd ..
print "Android lint 通过"

# 5. Android unit tests
print "[5/5] Android unit tests"
cd android
./gradlew test --quiet o> $"../($results_dir)/android-test.log"
cd ..
print "Android 单元测试通过"

print ""
print "=== 全部通过 ==="
print $"结果: ($results_dir)/"
