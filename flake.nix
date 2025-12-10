{
  description = "A desktop sticky image viewer for Wayland";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        version = "0.1.1";
      in
      {
        packages.default = pkgs.stdenv.mkDerivation {
          pname = "rspin";
          inherit version;

          src = pkgs.fetchurl {
            url = "https://github.com/dashu041120/rspin/releases/download/v${version}/rspin-${version}-x86_64-linux.tar.gz";
            sha256 = "1as5xjq9z4gdvpp2zh235626f5ndlsha245jl51rb1wk8b0pc64z";
          };

          nativeBuildInputs = [ pkgs.makeWrapper ];

          buildInputs = [
            pkgs.wayland
            pkgs.libxkbcommon
          ];

          unpackPhase = ''
            tar xzf $src
            cd rspin-${version}-x86_64-linux
          '';

          installPhase = ''
            mkdir -p $out/bin
            cp rspin $out/bin/
            chmod +x $out/bin/rspin

            # Wrap the binary to ensure runtime dependencies are available
            wrapProgram $out/bin/rspin \
              --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [
                pkgs.wayland
                pkgs.libxkbcommon
                pkgs.vulkan-loader
                pkgs.libGL
              ]}

            # Install documentation
            mkdir -p $out/share/doc/rspin
            cp README.md $out/share/doc/rspin/
          '';

          meta = with pkgs.lib; {
            description = "A desktop sticky image viewer for Wayland";
            homepage = "https://github.com/dashu041120/rspin";
            license = licenses.mit;
            platforms = [ "x86_64-linux" ];
            maintainers = [ ];
          };
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/rspin";
        };

        # Development shell for building from source
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust toolchain
            rustc
            cargo
            rustfmt
            clippy

            # Build dependencies
            pkg-config

            # Runtime dependencies
            wayland
            libxkbcommon
            vulkan-loader
            libGL

            # Optional tools
            wl-clipboard
          ];

          shellHook = ''
            echo "🦀 rspin development environment"
            echo "Usage:"
            echo "  cargo build          - Build the project"
            echo "  cargo run -- <args>  - Run rspin"
            echo "  cargo test           - Run tests"
            echo ""
            echo "Example:"
            echo "  cargo run -- /path/to/image.png"
          '';
        };
      }
    );
}
