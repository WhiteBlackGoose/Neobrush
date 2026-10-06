{
  description = "Neobrush - a modern cross-platform raster graphics editor";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        runtimeLibs = with pkgs; lib.optionals stdenv.isLinux [
          wayland
          libxkbcommon
          libGL
          vulkan-loader
          fontconfig
          freetype
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
          xorg.libxcb
        ];
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rustfmt clippy rust-analyzer pkg-config ];
          buildInputs = runtimeLibs;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "neobrush";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [ pkg-config makeWrapper ];
          buildInputs = runtimeLibs;
          postFixup = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            wrapProgram $out/bin/neobrush --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs}
          '';
        };
      });
}
