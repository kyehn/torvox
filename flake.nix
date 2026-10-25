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
              git
              flock
              nushell
              taplo
              yamlfmt
              markdownlint-cli2
              openspec
              mesa
              vulkan-loader
              vulkan-tools
              nixfmt-rs
              pkg-config
              openssl
              zig_0_17
              cargo-ndk
              (
                (pkgs.python3.pkgs.callPackage
                  (inputs.nixpkgs + "/pkgs/development/python-modules/semgrep/default.nix")
                  {
                    semgrep-core =
                      (pkgs.python3.pkgs.callPackage (
                        inputs.nixpkgs + "/pkgs/development/python-modules/semgrep/semgrep-core.nix"
                      ) { }).overrideAttrs
                        (oldAttrs: {
                          nativeBuildInputs =
                            pkgs.lib.lists.subtractLists [ pkgs.patchelf ] (oldAttrs.nativeBuildInputs or [ ])
                            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.patchelfUnstable ];
                          postFixup = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
                            ${pkgs.lib.getExe pkgs.patchelfUnstable} \
                              --set-rpath "$out/bin/libs:${pkgs.lib.makeLibraryPath [ pkgs.stdenv.cc.libc ]}" \
                              $out/bin/semgrep-core

                            ${pkgs.lib.getExe pkgs.patchelfUnstable} \
                              --set-interpreter "$(cat ${pkgs.stdenv.cc}/nix-support/dynamic-linker)" \
                              $out/bin/semgrep-core
                          '';
                        });
                  }
                ).overridePythonAttrs
                (oldAttrs: {
                  dependencies = map (
                    dependency: if dependency.pname == "mcp" then pkgs.python3.pkgs.mcp_1 else dependency
                  ) oldAttrs.dependencies;
                })
              )
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

                      install MapleMonoNormal-NF-CN-Regular.ttf -D --target-directory $out/share/fonts/truetype

                      runHook postInstall
                    '';
                  }))
                  noto-fonts-cjk-sans
                ];
              };
            };
            shellHook = ''
              set -e
              if [[ -n $NIX_GCROOT ]]; then
                  (
                    flock -x 9
                    repository_root=$(git rev-parse --show-toplevel)
                    chmod -R +x "$repository_root/scripts" "$repository_root/android/gradlew"
                    (cd "$repository_root" && nu scripts/download-aosp-testkey.nu)
                    nu "$repository_root/scripts/download-rapidocr-models.nu"
                    if [[ ! -d "/tmp/alacritty-theme" ]]; then
                      git clone --depth 1 --quiet https://github.com/alacritty/alacritty-theme.git /tmp/alacritty-theme
                    fi
                  ) 9>"/tmp/.terminal-devshell-$UID.lock"
              fi
            '';
          };
        };
    };
}
