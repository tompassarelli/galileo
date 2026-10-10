{
  description = "Galileo";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/cf3f5c4def3c7b5f1fc012b3d839575dbe552d43";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = [ pkgs.rustc pkgs.cargo pkgs.gcc pkgs.rustfmt pkgs.clippy pkgs.nix ];
        };
      });
    };
}
