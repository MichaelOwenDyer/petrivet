{
  description = "petrivet development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      mkChecks =
        pkgs:
        let
          inherit (pkgs) lib;

          # Only the cargo workspace, so editing e.g. flake.nix doesn't trigger a rebuild.
          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./petrivet
            ];
          };

          mkCargoCheck =
            name: command:
            pkgs.rustPlatform.buildRustPackage {
              pname = "petrivet-${name}";
              version = (lib.importTOML ./petrivet/Cargo.toml).package.version;
              inherit src;
              cargoLock.lockFile = ./Cargo.lock;

              # z3-sys locates libz3 via pkg-config, which emits a link-search path
              # that cargo forwards to both rustc and rustdoc (doctests).
              nativeBuildInputs = [ pkgs.pkg-config ];
              buildInputs = [ pkgs.z3 ];

              buildPhase = ''
                runHook preBuild
                ${command}
                runHook postBuild
              '';
              doCheck = false;
              installPhase = "touch $out";
            };
        in
        {
          tests = mkCargoCheck "tests" "cargo test";
          benches = mkCargoCheck "benches" "cargo bench --no-run";
        };
    in
    {
      checks = forAllSystems (system: mkChecks nixpkgs.legacyPackages.${system});

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.z3 ];
            packages = [
              pkgs.cargo
              pkgs.rustc
              pkgs.clippy
              pkgs.rustfmt
              pkgs.rust-analyzer
            ];
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };
        }
      );
    };
}
