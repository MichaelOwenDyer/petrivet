let
  pkgs = import <nixpkgs> { };
  src = pkgs.lib.cleanSourceWith {
    src = ../../.;
    filter = path: type:
      let
        name = builtins.baseNameOf path;
      in
        !(pkgs.lib.hasSuffix ".vmdk" name
          || pkgs.lib.hasSuffix ".qcow2" name
          || pkgs.lib.hasSuffix ".iso" name
          || name == "cache"
          || name == "artifacts"
          || name == "target"
          || name == ".git"
          || name == ".DS_Store");
  };
in
{
  petrivet-mcc = pkgs.pkgsCross.musl64.rustPlatform.buildRustPackage {
    pname = "petrivet-mcc";
    version = "0.1.0";
    inherit src;
    cargoLock.lockFile = ../../Cargo.lock;
    cargoBuildFlags = [ "-p" "petrivet-mcc" ];
    RUSTFLAGS = "-C target-feature=+crt-static";
    installPhase = ''
      runHook preInstall
      install -Dm755 target/x86_64-unknown-linux-musl/release/petrivet-mcc $out/bin/petrivet-mcc
      runHook postInstall
    '';
  };
}
