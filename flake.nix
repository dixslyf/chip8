{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      advisory-db,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) lib;

        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs = with pkgs; [
            alsa-lib
            libxkbcommon
            libGL
            vulkan-loader
            wayland
            xorg.libXcursor
            xorg.libXrandr
            xorg.libXi
            xorg.libX11
          ];

          nativeBuildInputs = with pkgs; [
            cmake
            pkg-config
            fontconfig
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        crate = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            doCheck = false;
          }
        );
      in
      {
        checks = {
          inherit crate;

          crate-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          crate-doc = craneLib.cargoDoc (
            commonArgs
            // {
              inherit cargoArtifacts;
            }
          );

          crate-fmt = craneLib.cargoFmt {
            inherit src;
          };

          crate-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          crate-deny = craneLib.cargoDeny {
            inherit src;
          };
        };

        packages = {
          default = crate;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = crate;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          # CMAKE_POLICY_VERSION_MINIMUM = "3.5"; # Needed for expat-sys crate
          LD_LIBRARY_PATH = "${lib.makeLibraryPath commonArgs.buildInputs}";
        };
      }
    );
}
