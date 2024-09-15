{
  description = "Concierge build and dev environment.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    compose2nix.url = "github:aksiksi/compose2nix";
    compose2nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, fenix, ... } @inputs:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rustTools = fenix.packages.${system};
        concierge = pkgs.rustPlatform.buildRustPackage {
          pname = "concierge";
          version = "0.2.0";
          src = pkgs.lib.cleanSource ./.;
          cargoHash = "sha256-jOgxOcawfu319XAZnnSAgT0B1oGIBrfcMaPU2803xaE=";
          nativeBuildInputs = with pkgs; [
            pkg-config
            gcc
            which
            inputs.compose2nix.packages.${system}.default
            libgit2
          ] ++ lib.optionals (stdenv.isDarwin) [
              darwin.apple_sdk.frameworks.Security
          ];
          buildInputs = with pkgs; [
            openssl
            libiconv
            libgit2
            inputs.compose2nix.packages.${system}.default
          ];

          # buildPhase = ''
          #   export PKG_CONFIG_PATH=${pkgs.libgit2.dev}/lib/pkgconfig:$PKG_CONFIG_PATH
          #   export LIBGIT2_STATIC=1
          #   export LIBGIT2_NO_PKG_CONFIG=1
          # '';
          RUST_BACKTRACE=1;
          RUST_DEBUG="debug";
        };
      in
      {
        packages.default = concierge;
        apps.default = {
          type = "app";
          program = "${concierge}/bin/concierge";
        };
        defaultPackage.${system} = concierge;
        defaultApp.${system} = self.apps.${system}.default;

        devShell = pkgs.mkShell {
          pure = true;
          buildInputs = with pkgs; [
            rustTools.default.toolchain
            bacon
            gcc
            pkg-config
            openssl
            rust-analyzer
            libiconv
            libgit2
            inputs.compose2nix.packages.${system}.default
          ];
        };
      });
}
