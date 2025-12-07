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
        version = "0.1.0";
      in
      {
        packages.default = pkgs.stdenv.mkDerivation {
          pname = "rspin";
          inherit version;

          src = pkgs.fetchurl {
            url = "https://github.com/dashu041120/rspin/releases/download/v${version}/rspin-${version}-x86_64-linux.tar.gz";
            sha256 = ""; # Users need to update this with the actual hash
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
      }
    );
}
