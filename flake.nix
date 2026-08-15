{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = inputs.nixpkgs.lib.systems.flakeExposed;
      perSystem =
        {
          pkgs,
          system,
          ...
        }:
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            config.allowUnfree = true;
            overlays = [
              inputs.fenix.overlays.default
              (final: prev: {
                # nixpkgs strictdoc 0.22.0 pulls Python 3.14 packages whose
                # pythonMetadataCheckPhase fails (importlib.metadata can't find
                # its own metadata after build): datauri 3.0.2 and the
                # tree-sitter grammar bindings. This is an upstream packaging
                # bug on Python 3.14, so skip the metadata check phase.
                # NOTE: the attribute in python3.pkgs is `datauri`, not
                # `python-datauri` (the latter name is the PyPI distribution).
                python3 = prev.python3.override {
                  packageOverrides = pself: pprev: {
                    datauri = pprev.datauri.overrideAttrs (old: {
                      dontCheck = true;
                      dontCheckPythonMetadata = true;
                    });
                    tree-sitter-grammars = pprev.tree-sitter-grammars // {
                      tree-sitter-python = pprev.tree-sitter-grammars.tree-sitter-python.overrideAttrs (old: {
                        dontCheckPythonMetadata = true;
                      });
                      tree-sitter-rust = pprev.tree-sitter-grammars.tree-sitter-rust.overrideAttrs (old: {
                        dontCheckPythonMetadata = true;
                      });
                      tree-sitter-cpp = pprev.tree-sitter-grammars.tree-sitter-cpp.overrideAttrs (old: {
                        dontCheckPythonMetadata = true;
                      });
                    };
                  };
                };
              })
            ];
          };
          packages.rust-toolchain-latest = pkgs.fenix.combine [
            (pkgs.fenix.latest.withComponents [
              "cargo"
              "clippy"
              "rust-src"
              "rustc"
              "rustfmt"
            ])
            pkgs.fenix.targets.thumbv6m-none-eabi.latest.rust-std
            pkgs.fenix.targets.x86_64-linux-android.latest.rust-std
            pkgs.fenix.targets.aarch64-linux-android.latest.rust-std
          ];
          formatter = pkgs.nixfmt-tree.override {
            nixfmtPackage = pkgs.nixfmt-rs;
            runtimeInputs = with pkgs; [
              taplo
              yamlfmt
              rustfmt
              typos
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
              rustfmt = {
                command = "rustfmt";
                options = [
                  "--config"
                  "skip_children=true"
                  "--edition"
                  "2024"
                  "--style-edition"
                  "2024"
                ];
                includes = [ "*.rs" ];
              };
              typos = {
                command = "typos";
                includes = [
                  "*.rs"
                  "*.kt"
                  "*.md"
                  "*.toml"
                  "*.nix"
                  "*.yml"
                  "*.yaml"
                ];
              };
            };
          };
          devShells.default = pkgs.mkShell {
            name = "default";
            packages = with pkgs; [
              (fenix.combine [
                (fenix.stable.withComponents [
                  "cargo"
                  "clippy"
                  "rust-src"
                  "rustc"
                  "rustfmt"
                ])
                fenix.targets.thumbv6m-none-eabi.stable.rust-std
                fenix.targets.x86_64-linux-android.stable.rust-std
                fenix.targets.aarch64-linux-android.stable.rust-std
              ])
              cargo-fuzz
              cargo-geiger
              cargo-audit
              cargo-machete
              cargo-llvm-cov
              adrs
              # strictdoc 0.22.0 builds against the overridden python3 (see
              # _module.args.pkgs overlay: datauri check skipped on Py 3.14).
              (strictdoc.override { python3 = pkgs.python3; })
              kotlin
              gradle_9
              jdk
              ktfmt
              ktlint
              android-tools
              # nix git (not the system /usr/bin/git): the system git's https
              # helper is incompatible with the devshell glibc (LD_LIBRARY_PATH),
              # which breaks `git clone` inside cargo build scripts
              # (libghostty-vt-sys downloads ghostty at build time).
              git
              nushell
              taplo
              yamlfmt
              typos
              vale
              markdownlint-cli2
              mesa
              mold
              vulkan-loader
              vulkan-tools
              nixfmt-rs
              pkg-config
              openssl
              zig_0_15
              cargo-ndk
              maestro
              semgrep
              systemdLibs
              fontconfig
              (maple-mono.Normal-NF-CN.overrideAttrs (_: {
                installPhase = ''
                  runHook preInstall

                  install MapleMonoNormal-NF-CN-Medium.ttf -D --target-directory $out/share/fonts/truetype

                  runHook postInstall
                '';
              }))
              libpulseaudio
              (lib.getLib stdenv.cc.cc)
              (python3.withPackages (
                ps: with ps; [
                  pip
                  (rapidocr.overridePythonAttrs (oldAttrs: {
                    postPatch = (oldAttrs.postPatch or "") + ''
                      substituteInPlace rapidocr/config.yaml \
                        --replace-fail "model_root_dir: null" "model_root_dir: /tmp/.rapidocr-models"
                      substituteInPlace rapidocr/utils/parse_parameters.py \
                        --replace-fail "cfg = OmegaConf.load(file_path)" "cfg = OmegaConf.load(file_path if file_path else str(Path(__file__).parent.parent / 'config.yaml'))"
                    '';
                  }))
                ]
              ))
            ];
            env = {
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (
                with pkgs;
                [
                  pkg-config
                  openssl
                  vulkan-loader
                  mesa
                  stdenv.cc.cc
                  libpulseaudio
                ]
              );
              VK_ICD_FILENAMES = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
            };
            shellHook = ''
              set -e
              # libghostty-vt-sys 0.2.1 pins ghostty a887df42, whose build.zig
              # requires Zig 0.15.2 — nixpkgs `zig_0_15` is exactly that.
              export PATH="${pkgs.lib.makeBinPath [ pkgs.zig_0_15 ]}:$PATH"
              nu scripts/fetch-aosp-testkey.nu
              nu scripts/download-rapidocr-models.nu
            '';
          };
        };
    };
}
