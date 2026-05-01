let
  pkgs = import <nixpkgs> { };
  # Every binary the orchestrator's `host_command` shells out to. Keeping them
  # listed centrally here means the same set is pinned for both `nix-shell`
  # entry and the `run` wrapper, so behavior stays identical regardless of
  # whether a developer launched the orchestrator from `nix-shell` directly
  # or from the wrapped binary in their PATH.
  hostDeps = with pkgs; [
    qemu       # qemu-system-x86_64, qemu-img
    openssh    # ssh, scp
    procps     # kill, pkill
    gnutar     # tar (smoke-input archive packaging)
    coreutils  # chmod, mkdir, rm, ...
    nix        # the orchestrator itself shells out to `nix build`
  ];
in
{
  # Drop into a host shell with every binary the orchestrator needs:
  #
  #   nix-shell mcc-2026/mcc-orchestrator -A shell
  #   cargo run --bin mcc-orchestrator
  #
  # This works the same on macOS, Linux, and NixOS; the orchestrator's
  # `host_command` helper finds each tool on PATH and skips the per-call
  # `nix shell` fallback.
  shell = pkgs.mkShell {
    name = "mcc-orchestrator-shell";
    buildInputs = hostDeps ++ [ pkgs.cargo pkgs.rustc ];
    shellHook = ''
      export ORCHESTRATOR_NIX_SHELL=1
      echo "mcc-orchestrator host shell: qemu/ssh/scp/tar are on PATH"
    '';
  };

  # Re-export host dependencies so callers can build their own wrappers
  # without copying the list. e.g. `(import ./default.nix).hostDeps`.
  inherit hostDeps;
}
