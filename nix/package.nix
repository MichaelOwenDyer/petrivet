# Builds the `petrivet` command-line tool. z3 is the SMT backend and is linked dynamically.
{ lib, rustPlatform, pkg-config, z3 }:

rustPlatform.buildRustPackage {
  pname = "petrivet";
  version = "0.1.0";

  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type:
      let name = baseNameOf path;
      in !(builtins.elem name [ "target" ".git" ".idea" ".DS_Store" "docs" "result" ]);
  };

  cargoLock.lockFile = ../Cargo.lock;

  cargoBuildFlags = [ "-p" "petrivet-cli" ];
  cargoTestFlags = [ "-p" "petrivet-cli" ];

  nativeBuildInputs = [ pkg-config rustPlatform.bindgenHook ];
  buildInputs = [ z3 ];

  meta = {
    description = "Place/transition Petri net analyzer";
    mainProgram = "petrivet";
    license = lib.licenses.lgpl3Plus;
  };
}
