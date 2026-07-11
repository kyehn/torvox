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
            overlays = [ inputs.fenix.overlays.default ];
          };
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
              cargo-nextest
              cargo-fuzz
              cargo-llvm-cov
              cargo-geiger
              cargo-audit
              cargo-deny
              cargo-machete
              cargo-mutants
              rust-analyzer
              kotlin
              gradle_9
              jdk
              ktfmt
              ktlint
              android-tools
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
              statix
              deadnix
              pkg-config
              openssl
              zig_0_15
              cargo-ndk
              maestro
              semgrep
              systemd
              imagemagick
              fontconfig
              noto-fonts-cjk-sans
              libpulseaudio
              (lib.getLib stdenv.cc.cc)
              (python3.withPackages (
                ps: with ps; [
                  pyte
                  sphinx
                  sphinx-rtd-theme
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
              git
              curl
              jq
              gnutar
              gzip
              patch
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
              export PATH="${pkgs.lib.makeBinPath [ pkgs.zig_0_15 ]}:$PATH"
              export GHOSTTY_SOURCE_DIR="$(nu scripts/bootstrap-libghostty.nu | tail -1)"
              nu scripts/fetch-aosp-testkey.nu
              nu scripts/download-rapidocr-models.nu
            '';
          };
        };
    };
}
