{
  description = "Typst environment for petrivet masters thesis";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      mkPkgs = system: nixpkgs.legacyPackages.${system};
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = mkPkgs system;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              typst
              texliveBasic
              tex-gyre
            ];
            shellHook = ''
              export TYPST_FONT_PATHS="${pkgs.tex-gyre}/opentype"
              mkdir -p out
              echo "Typst thesis shell ready."
              echo "  make        # compile to out/thesis.pdf"
              echo "  make watch  # live reload"
            '';
          };
        });
    };
}
