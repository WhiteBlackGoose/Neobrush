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
        lib = pkgs.lib;
        isLinux = pkgs.stdenv.hostPlatform.isLinux;
        x = name: old: pkgs.${name} or pkgs.xorg.${old};
        runtimeLibs = lib.optionals isLinux [
          pkgs.wayland
          pkgs.libxkbcommon
          pkgs.libGL
          pkgs.vulkan-loader
          pkgs.fontconfig
          pkgs.freetype
          (x "libx11" "libX11")
          (x "libxcursor" "libXcursor")
          (x "libxrandr" "libXrandr")
          (x "libxi" "libXi")
          (x "libxcb" "libxcb")
        ];
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rustfmt clippy rust-analyzer pkg-config ];
          buildInputs = runtimeLibs;
          LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "neobrush";
          version = "0.3.1-alpha";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [ pkg-config makeWrapper ];
          buildInputs = runtimeLibs;
          doCheck = false;
          postInstall = lib.optionalString isLinux ''
            install -Dm644 packaging/neobrush.desktop $out/share/applications/neobrush.desktop
            install -Dm644 packaging/neobrush.svg $out/share/icons/hicolor/scalable/apps/neobrush.svg
          '';
          postFixup = lib.optionalString isLinux ''
            wrapProgram $out/bin/neobrush --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibs}
          '';
          meta = {
            description = "A modern, cross-platform raster graphics editor";
            license = lib.licenses.mit;
            mainProgram = "neobrush";
          };
        };

        apps.default = flake-utils.lib.mkApp { drv = self.packages.${system}.default; };
      });
}
