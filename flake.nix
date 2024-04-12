{
  description = "Ultralang build and dev environment.";

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
      in
      {
        devShell = pkgs.mkShell {
          buildInputs = [
            rustTools.default.toolchain
            pkgs.bacon
            pkgs.zsh
            pkgs.gcc
            pkgs.pkg-config
            pkgs.openssl
          ];
        };
      });
}
