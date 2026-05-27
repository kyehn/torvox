{
  description = "Torvox — Android 终端模拟器";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    fenix.url = "github:nix-community/fenix";
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem =
        { pkgs, system, ... }:
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            config.allowUnfree = true;
            overlays = [ inputs.fenix.overlays.default ];
          };

          packages.rust-toolchain = pkgs.fenix.stable.withComponents [
            "cargo"
            "clippy"
            "rust-src"
            "rustc"
            "rustfmt"
          ];

          # ── 格式化器 ──────────────────────────────────────
          formatter = pkgs.nixfmt-tree.override {
            nixfmtPackage = pkgs.nixfmt-rs;
            runtimeInputs = [
              pkgs.taplo
              pkgs.yamlfmt
              pkgs.shfmt
            ];
            settings.formatter = {
              toml = {
                command = "taplo";
                options = [ "format" ];
                includes = [ "*.toml" ];
              };
              yaml = {
                command = "yamlfmt";
                includes = [
                  "*.yaml"
                  "*.yml"
                ];
              };
              shell = {
                command = "shfmt";
                options = [
                  "-w"
                  "-i"
                  "2"
                  "-ci"
                ];
                includes = [
                  "*.sh"
                  "*.bash"
                ];
              };
            };
          };

          # ── 质量检查 ──────────────────────────────────────
          checks =
            let
              toolchain = pkgs.fenix.stable.withComponents [
                "cargo"
                "clippy"
                "rust-src"
                "rustc"
                "rustfmt"
              ];
              native-dependencies = [
                toolchain
                pkgs.cargo-nextest
                pkgs.pkg-config
                pkgs.openssl
                pkgs.zig_0_15
              ];
              copy-source = "cp -r ${./.} . && chmod -R u+w .";
            in
            {
              clippy =
                pkgs.runCommand "check-clippy"
                  {
                    nativeBuildInputs = native-dependencies;
                    RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
                    LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
                      pkgs.pkg-config
                      pkgs.openssl
                    ];
                  }
                  ''
                    ${copy-source}
                    cargo clippy -- -D warnings
                    touch $out
                  '';

              fmt =
                pkgs.runCommand "check-fmt"
                  {
                    nativeBuildInputs = [ toolchain ];
                  }
                  ''
                    ${copy-source}
                    cargo fmt --check
                    touch $out
                  '';

              tests =
                pkgs.runCommand "check-tests"
                  {
                    nativeBuildInputs = native-dependencies;
                    RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
                    LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
                      pkgs.pkg-config
                      pkgs.openssl
                    ];
                  }
                  ''
                    ${copy-source}
                    cargo nextest run --workspace
                    touch $out
                  '';

              proptest =
                pkgs.runCommand "check-proptest"
                  {
                    nativeBuildInputs = native-dependencies;
                    RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
                    LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
                      pkgs.pkg-config
                      pkgs.openssl
                    ];
                  }
                  ''
                    ${copy-source}
                    cargo test --workspace -- proptest
                    touch $out
                  '';

              typos =
                pkgs.runCommand "check-typos"
                  {
                    nativeBuildInputs = [ pkgs.typos ];
                  }
                  ''
                    ${copy-source}
                    typos
                    touch $out
                  '';

              nixfmt =
                pkgs.runCommand "check-nixfmt"
                  {
                    nativeBuildInputs = [ pkgs.nixfmt-rs ];
                  }
                  ''
                    ${copy-source}
                    find . -name '*.nix' \
                      -not -path './target/*' \
                      -not -path './.git/*' \
                      -exec nixfmt --check {} +
                    touch $out
                  '';

              cargo-deny =
                pkgs.runCommand "check-cargo-deny"
                  {
                    nativeBuildInputs = [
                      toolchain
                      pkgs.cargo-deny
                    ];
                  }
                  ''
                    ${copy-source}
                    cargo deny check
                    touch $out
                  '';

              cargo-audit =
                pkgs.runCommand "check-cargo-audit"
                  {
                    nativeBuildInputs = [
                      toolchain
                      pkgs.cargo-audit
                    ];
                  }
                  ''
                    ${copy-source}
                    cargo audit
                    touch $out
                  '';

              cargo-machete =
                pkgs.runCommand "check-cargo-machete"
                  {
                    nativeBuildInputs = [
                      toolchain
                      pkgs.cargo-machete
                    ];
                  }
                  ''
                    ${copy-source}
                    cargo machete
                    touch $out
                  '';

              markdownlint =
                pkgs.runCommand "check-markdownlint"
                  {
                    nativeBuildInputs = [
                      pkgs.nodePackages.markdownlint-cli
                    ];
                  }
                  ''
                    ${copy-source}
                    find . -name '*.md' \
                      -not -path './target/*' \
                      -not -path './.git/*' \
                      -not -path './.opencode/*' \
                      -exec markdownlint {} +
                    touch $out
                  '';
            };

          # ── 开发环境 ──────────────────────────────────────
          devShells.default = pkgs.mkShell {
            name = "torvox-dev";
            packages = [
              # Rust
              (pkgs.fenix.stable.withComponents [
                "cargo"
                "clippy"
                "rust-src"
                "rustc"
                "rustfmt"
              ])
              pkgs.cargo-nextest
              pkgs.cargo-fuzz
              pkgs.cargo-geiger
              pkgs.cargo-audit
              pkgs.cargo-ndk
              pkgs.cargo-deny
              pkgs.cargo-machete
              pkgs.rust-analyzer

              # Zig (Ghostty VT 构建依赖)
              pkgs.zig_0_15

              # Android
              pkgs.kotlin
              pkgs.gradle_9
              pkgs.ktfmt
              pkgs.ktlint
              pkgs.android-tools

              # 代码质量
              pkgs.nushell
              pkgs.taplo
              pkgs.yamlfmt
              pkgs.shfmt
              pkgs.typos
              pkgs.nodePackages.markdownlint-cli

              # 原生依赖
              pkgs.pkg-config
              pkgs.openssl
            ];
            env = {
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
                pkgs.pkg-config
                pkgs.openssl
              ];
            };
          };
        };
    };
}
