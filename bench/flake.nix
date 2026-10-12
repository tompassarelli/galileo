{
  description = "Seven-tool language comparison bench shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/02d1c9ad58d56732a5ae2412981aca62ac4777fa";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAllSystems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [ nickel cue dhall dhall-json kcl nixfmt hyperfine ];
          };
        });
    };
}
