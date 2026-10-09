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
          packages.rust-toolchain-latest = pkgs.fenix.combine [
            (pkgs.fenix.latest.withComponents [
              "cargo"
              "clippy"
              "rust-src"
              "rustc"
              "rustfmt"
            ])
            pkgs.fenix.targets.x86_64-linux-android.latest.rust-std
            pkgs.fenix.targets.aarch64-linux-android.latest.rust-std
          ];
          formatter = pkgs.nixfmt-tree.override {
            nixfmtPackage = pkgs.nixfmt-rs;
            runtimeInputs = with pkgs; [
              taplo
              yamlfmt
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
                fenix.targets.x86_64-linux-android.stable.rust-std
                fenix.targets.aarch64-linux-android.stable.rust-std
              ])
              cargo-machete
              kotlin
              gradle
              jdk
              android-tools
              curl
              git
              nushell
              taplo
              yamlfmt
              markdownlint-cli2
              mesa
              vulkan-loader
              vulkan-tools
              nixfmt-rs
              pkg-config
              openssl
              zig_0_16
              cargo-ndk
              (semgrep.overridePythonAttrs (oldAttrs: {
                pythonRelaxDeps = true;
              }))
              systemdLibs
              fontconfig
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
              FONTCONFIG_FILE = pkgs.makeFontsConf {
                fontDirectories = with pkgs; [
                  (maple-mono.Normal-NF-CN.overrideAttrs (_: {
                    installPhase = ''
                      runHook preInstall

                      install MapleMonoNormal-NF-CN-Medium.ttf -D --target-directory $out/share/fonts/truetype

                      runHook postInstall
                    '';
                  }))
                  noto-fonts-cjk-sans
                ];
              };
            };
            shellHook = ''
              set -e
              chmod -R +x scripts/ android/gradlew
              nu scripts/download-aosp-testkey.nu
              nu scripts/download-rapidocr-models.nu
              if [[ ! -d "/tmp/alacritty-theme" ]]; then
                git clone --depth 1 --quiet https://github.com/alacritty/alacritty-theme.git /tmp/alacritty-theme
              fi
              # UAX #29 字素簇语料：固定 Unicode 18.0.0，不跟随 latest，
              # 否则 UCD 每年发版会让测试在无人审阅时变红。
              if [[ ! -f "/tmp/unicode-ucd/GraphemeBreakTest.txt" ]]; then
                curl --fail --silent --show-error --location --create-dirs \
                  --output /tmp/unicode-ucd/GraphemeBreakTest.txt \
                  https://www.unicode.org/Public/18.0.0/ucd/auxiliary/GraphemeBreakTest.txt
              fi
              # Kitty 图像载荷：取自 libghostty-vt-sys 构建时所用的 ghostty 版本，
              # 使载荷与实际链接的引擎一致；该 rev 由该依赖的 build.rs 固定。
              if [[ ! -d "/tmp/ghostty-kitty-testdata" ]]; then
                curl --fail --silent --show-error --location --create-dirs \
                  --output /tmp/ghostty-kitty-testdata/image-rgb-none-20x15-2147483647-raw.data \
                  https://raw.githubusercontent.com/ghostty-org/ghostty/22d13172cde98a0a4dda05d3d6a3fcb0dd8ed018/src/terminal/kitty/testdata/image-rgb-none-20x15-2147483647-raw.data
                curl --fail --silent --show-error --location --create-dirs \
                  --output /tmp/ghostty-kitty-testdata/image-rgb-zlib_deflate-128x96-2147483647-raw.data \
                  https://raw.githubusercontent.com/ghostty-org/ghostty/22d13172cde98a0a4dda05d3d6a3fcb0dd8ed018/src/terminal/kitty/testdata/image-rgb-zlib_deflate-128x96-2147483647-raw.data
              fi
            '';
          };
        };
    };
}
