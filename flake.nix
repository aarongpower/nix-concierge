{
  description = "Concierge build and dev environment.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, fenix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rustTools = fenix.packages.${system};
        concierge = pkgs.rustPlatform.buildRustPackage {
          pname = "concierge";
          version = "0.1.0";
          src = pkgs.lib.cleanSource ./.;
          cargoSha256 = "sha256-IX339g7h62uatu45BD11y/vkDMQQezNuk4hSbwylgu4=";
          buildInputs = with pkgs; [
            openssl
            libiconv
            libgit2
            pkg-config
          ];
        };
      in
      {
        packages.default = concierge;
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustTools.default.toolchain
            bacon
            zsh
            gcc
            pkg-config
            openssl
            rust-analyzer
            libiconv
            libgit2
          ];
        };
      });
}
