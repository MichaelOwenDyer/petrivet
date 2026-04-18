use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let paths = Paths::discover()?;
    paths.prepare()?;
    build_petrivet_mcc(&paths)?;
    build_submission_image(&paths)?;
    Ok(())
}

struct Paths {
    orchestrator_dir: PathBuf,
    runtime_cache_dir: PathBuf,
    artifacts_dir: PathBuf,
    built_binary: PathBuf,
    runtime_work_image: PathBuf,
    runtime_output_image: PathBuf,
    runtime_benchkit_head: PathBuf,
    base_vm_image: PathBuf,
    input_vm_image: PathBuf,
    contest_key: PathBuf,
}

impl Paths {
    fn discover() -> Result<Self, String> {
        let orchestrator_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if !orchestrator_dir.is_dir() {
            return Err(format!(
                "missing orchestrator directory: {}",
                orchestrator_dir.display()
            ));
        }

        let base_vm_image = env_path("MCC_BASE_IMAGE")
            .unwrap_or_else(|| orchestrator_dir.join("SubmissionKit/mcc2026.vmdk"));
        let input_vm_image = env_path("MCC_INPUT_IMAGE")
            .unwrap_or_else(|| orchestrator_dir.join("SubmissionKit/mcc2026-input.vmdk"));
        let contest_key = env_path("MCC_CONTEST_KEY")
            .unwrap_or_else(|| orchestrator_dir.join("SubmissionKit/bk-private_key"));
        let runtime_cache_dir = orchestrator_dir.join("cache").join("runtime");
        let artifacts_dir = orchestrator_dir.join("artifacts");

        Ok(Self {
            orchestrator_dir,
            runtime_cache_dir: runtime_cache_dir.clone(),
            artifacts_dir: artifacts_dir.clone(),
            built_binary: artifacts_dir.join("petrivet-2026"),
            runtime_work_image: runtime_cache_dir.join("petrivet-2026-runtime.vmdk"),
            runtime_output_image: artifacts_dir.join("petrivet-2026.vmdk"),
            runtime_benchkit_head: runtime_cache_dir.join("BenchKit_head.sh"),
            base_vm_image,
            input_vm_image,
            contest_key,
        })
    }

    fn prepare(&self) -> Result<(), String> {
        self.require_existing(&self.orchestrator_dir, "orchestrator directory")?;
        self.require_existing(&self.base_vm_image, "boot VM image")?;
        self.require_existing(&self.input_vm_image, "input VM image")?;
        self.require_existing(&self.contest_key, "contest SSH key")?;

        fs::create_dir_all(&self.runtime_cache_dir)
            .map_err(|error| format!("failed to create runtime cache: {error}"))?;
        fs::create_dir_all(&self.artifacts_dir)
            .map_err(|error| format!("failed to create artifacts directory: {error}"))?;
        self.write_runtime_benchkit_head()?;
        Ok(())
    }

    fn require_existing(&self, path: &Path, label: &str) -> Result<(), String> {
        if path.exists() {
            return Ok(());
        }
        Err(format!("missing {label}: {}", path.display()))
    }

    fn write_runtime_benchkit_head(&self) -> Result<(), String> {
        fs::write(&self.runtime_benchkit_head, BENCHKIT_HEAD_SCRIPT)
            .map_err(|error| format!("failed to write BenchKit_head.sh: {error}"))?;
        set_executable(&self.runtime_benchkit_head)?;
        Ok(())
    }
}

fn build_petrivet_mcc(paths: &Paths) -> Result<(), String> {
    println!("building petrivet-mcc with nix build...");
    let mut command = Command::new("nix");
    let nix_file = paths.orchestrator_dir.join("default.nix");
    if !nix_file.is_file() {
        return Err(format!("missing Nix build file: {}", nix_file.display()));
    }
    command
        .arg("build")
        .arg("--no-link")
        .arg("--print-out-paths")
        .arg("--file")
        .arg(&nix_file)
        .arg("petrivet-2026");
    let output = run_command_streaming_stdout(command, "build petrivet-mcc")?;
    let package_out = output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "nix build produced no output path".to_string())?;
    let package_out = PathBuf::from(package_out.trim());
    let built_binary = package_out.join("bin").join("petrivet-mcc");
    if !built_binary.is_file() {
        return Err(format!(
            "nix build finished but binary is missing: {}",
            built_binary.display()
        ));
    }
    fs::copy(&built_binary, &paths.built_binary).map_err(|error| {
        format!(
            "failed to copy built binary into {}: {error}",
            paths.built_binary.display()
        )
    })?;
    set_executable(&paths.built_binary)?;
    if !paths.built_binary.is_file() {
        return Err(format!(
            "build finished but binary is missing: {}",
            paths.built_binary.display()
        ));
    }
    Ok(())
}

fn build_submission_image(paths: &Paths) -> Result<(), String> {
    println!("preparing runtime VM copy...");
    cleanup_runtime_guest(paths)?;
    if paths.runtime_work_image.exists() {
        fs::remove_file(&paths.runtime_work_image).map_err(|error| {
            format!(
                "failed to remove old runtime work image {}: {error}",
                paths.runtime_work_image.display()
            )
        })?;
    }
    if paths.runtime_output_image.exists() {
        fs::remove_file(&paths.runtime_output_image).map_err(|error| {
            format!(
                "failed to remove old submission image {}: {error}",
                paths.runtime_output_image.display()
            )
        })?;
    }

    create_runtime_overlay(paths)?;

    let runtime = RuntimeSession::start(paths)?;
    runtime.wait_for_ssh()?;
    runtime.install_payloads()?;
    runtime.smoke_test()?;
    runtime.shutdown()?;

    flatten_runtime_overlay(paths)?;

    println!("done");
    println!("  binary: {}", paths.built_binary.display());
    println!("  submission image: {}", paths.runtime_output_image.display());
    Ok(())
}

fn cleanup_runtime_guest(paths: &Paths) -> Result<(), String> {
    let mut cleanup_attempted = false;
    if paths.runtime_cache_dir.join("runtime.qemu.pid").exists() {
        cleanup_attempted = true;
        if let Ok(pid_text) = fs::read_to_string(paths.runtime_cache_dir.join("runtime.qemu.pid")) {
            if let Ok(pid) = pid_text.trim().parse::<i32>() {
                let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
            }
        }
        let _ = fs::remove_file(paths.runtime_cache_dir.join("runtime.qemu.pid"));
    }

    let mut pkill = host_command("pkill", &["procps"]);
    pkill.arg("-f").arg("hostfwd=tcp::2222-:22");
    let _ = pkill.status();

    if cleanup_attempted {
        thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}

struct RuntimeSession<'a> {
    paths: &'a Paths,
    ssh_port: u16,
    vnc_port: u16,
    pid_file: PathBuf,
}

impl<'a> RuntimeSession<'a> {
    fn start(paths: &'a Paths) -> Result<Self, String> {
        let session = Self {
            paths,
            ssh_port: 2222,
            vnc_port: 42,
            pid_file: paths.runtime_cache_dir.join("runtime.qemu.pid"),
        };
        if session.pid_file.exists() {
            fs::remove_file(&session.pid_file).map_err(|error| {
                format!(
                    "failed to remove stale pid file {}: {error}",
                    session.pid_file.display()
                )
            })?;
        }

        println!("starting runtime VM copy...");
        let mut command = host_command("qemu-system-x86_64", &["qemu"]);
        command
            .arg("-machine")
            .arg("q35,accel=tcg")
            .arg("-m")
            .arg("4096")
            .arg("-smp")
            .arg("1")
            .arg("-cpu")
            .arg("Westmere")
            .arg("-daemonize")
            .arg("-pidfile")
            .arg(&session.pid_file)
            .arg("-blockdev")
            .arg(format!(
                "driver=file,filename={},node-name=system-file,read-only=off",
                session.paths.runtime_work_image.display()
            ))
            .arg("-blockdev")
            .arg("driver=qcow2,file=system-file,node-name=system,read-only=off")
            .arg("-device")
            .arg("virtio-blk-pci,drive=system")
            .arg("-blockdev")
            .arg(format!(
                "driver=file,filename={},node-name=input-file,read-only=on",
                session.paths.input_vm_image.display()
            ))
            .arg("-blockdev")
            .arg("driver=vmdk,file=input-file,node-name=input,read-only=on")
            .arg("-device")
            .arg("virtio-blk-pci,drive=input")
            .arg("-net")
            .arg("nic")
            .arg("-net")
            .arg(format!("user,hostfwd=tcp::{}-:22", session.ssh_port))
            .arg("-vnc")
            .arg(format!(":{}", session.vnc_port));

        run_command(command, "start runtime VM")?;
        Ok(session)
    }

    fn wait_for_ssh(&self) -> Result<(), String> {
        println!("waiting for runtime SSH on port {}...", self.ssh_port);
        for _ in 0..180 {
            if run_ssh_probe(self.paths, self.ssh_port, "mcc").is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_secs(2));
        }
        Err("runtime VM did not become ready".to_string())
    }

    fn install_payloads(&self) -> Result<(), String> {
        println!("installing submission files into runtime VM...");
        run_ssh_as(
            self.paths,
            self.ssh_port,
            "root",
            "mkdir -p /home/mcc/BenchKit/bin /home/mcc/BenchKit/INPUTS && chown -R mcc /home/mcc/BenchKit",
        )?;
        run_scp_as(
            self.paths,
            self.ssh_port,
            "root",
            &self.paths.runtime_benchkit_head,
            "/home/mcc/BenchKit/BenchKit_head.sh",
        )?;
        run_scp_as(
            self.paths,
            self.ssh_port,
            "root",
            &self.paths.built_binary,
            "/home/mcc/BenchKit/bin/petrivet-mcc",
        )?;
        run_ssh_as(
            self.paths,
            self.ssh_port,
            "root",
            "chmod +x /home/mcc/BenchKit/BenchKit_head.sh /home/mcc/BenchKit/bin/petrivet-mcc",
        )?;
        Ok(())
    }

    fn smoke_test(&self) -> Result<(), String> {
        println!("sanity-checking entrypoint in runtime VM...");
        run_ssh(
            self.paths,
            self.ssh_port,
            "BK_TOOL=petrivet-mcc BK_EXAMINATION=StateSpace BIN_DIR=/home/mcc/BenchKit/bin /home/mcc/BenchKit/BenchKit_head.sh",
        )
    }

    fn shutdown(self) -> Result<(), String> {
        run_ssh_as(
            self.paths,
            self.ssh_port,
            "root",
            r#"sh -lc 'if [ -x /usr/sbin/poweroff ]; then nohup /usr/sbin/poweroff >/dev/null 2>&1 </dev/null & exit 0; fi; if [ -x /sbin/poweroff ]; then nohup /sbin/poweroff >/dev/null 2>&1 </dev/null & exit 0; fi; if [ -x /bin/systemctl ]; then nohup /bin/systemctl poweroff >/dev/null 2>&1 </dev/null & exit 0; fi; if [ -x /usr/bin/systemctl ]; then nohup /usr/bin/systemctl poweroff >/dev/null 2>&1 </dev/null & exit 0; fi; exit 127'"#,
        )?;
        wait_for_pid_to_exit(&self.pid_file, 120)
    }
}

fn run_ssh(paths: &Paths, port: u16, command: &str) -> Result<(), String> {
    run_ssh_as(paths, port, "mcc", command)
}

fn run_ssh_probe(paths: &Paths, port: u16, user: &str) -> Result<(), String> {
    let status = Command::new("ssh")
        .arg("-q")
        .arg("-o")
        .arg("LogLevel=ERROR")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=3")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-i")
        .arg(&paths.contest_key)
        .arg("-p")
        .arg(port.to_string())
        .arg(format!("{user}@localhost"))
        .arg("true")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to probe ssh as {user} on port {port}: {error}"))?;

    ensure_success(&format!("ssh probe as {user} on port {port}"), status)
}

fn run_ssh_as(paths: &Paths, port: u16, user: &str, command: &str) -> Result<(), String> {
    let status = Command::new("ssh")
        .arg("-o")
        .arg("LogLevel=ERROR")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("ConnectTimeout=3")
        .arg("-i")
        .arg(&paths.contest_key)
        .arg("-p")
        .arg(port.to_string())
        .arg(format!("{user}@localhost"))
        .arg(command)
        .status()
        .map_err(|error| format!("failed to run ssh as {user} on port {port}: {error}"))?;

    ensure_success(&format!("ssh command as {user} on port {port}"), status)
}

fn run_scp_as(paths: &Paths, port: u16, user: &str, local: &Path, remote: &str) -> Result<(), String> {
    let status = Command::new("scp")
        .arg("-q")
        .arg("-o")
        .arg("LogLevel=ERROR")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-i")
        .arg(&paths.contest_key)
        .arg("-P")
        .arg(port.to_string())
        .arg(local)
        .arg(format!("{user}@localhost:{remote}"))
        .status()
        .map_err(|error| format!("failed to run scp as {user} to port {port}: {error}"))?;

    ensure_success(&format!("scp as {user} to port {port}"), status)
}

fn wait_for_pid_to_exit(pid_file: &Path, deadline_seconds: u64) -> Result<(), String> {
    let pid = fs::read_to_string(pid_file)
        .map_err(|error| format!("failed to read pid file {}: {error}", pid_file.display()))?;
    let pid = pid.trim();
    if pid.is_empty() {
        return Ok(());
    }

    let pid = pid
        .parse::<i32>()
        .map_err(|error| format!("invalid pid in {}: {error}", pid_file.display()))?;
    for _ in 0..deadline_seconds {
        if !process_exists(pid) {
            return Ok(());
        }
        thread::sleep(Duration::from_secs(1));
    }
    Err(format!(
        "QEMU process {pid} still appears to be running after {deadline_seconds}s"
    ))
}

fn process_exists(pid: i32) -> bool {
    let mut command = Command::new("kill");
    command.arg("-0").arg(pid.to_string());
    matches!(command.status(), Ok(status) if status.success())
}

fn create_runtime_overlay(paths: &Paths) -> Result<(), String> {
    let mut command = host_command("qemu-img", &["qemu"]);
    command
        .arg("convert")
        .arg("-O")
        .arg("qcow2")
        .arg(&paths.base_vm_image)
        .arg(&paths.runtime_work_image);
    run_command(command, "create runtime overlay")
}

fn flatten_runtime_overlay(paths: &Paths) -> Result<(), String> {
    let mut command = host_command("qemu-img", &["qemu"]);
    command
        .arg("convert")
        .arg("-O")
        .arg("vmdk")
        .arg(&paths.runtime_work_image)
        .arg(&paths.runtime_output_image);
    run_command(command, "flatten runtime overlay")
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn run_command(mut command: Command, label: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("failed to start {label}: {error}"))?;
    ensure_success(label, status)
}

fn host_command(program: &str, nix_packages: &[&str]) -> Command {
    if program_on_path(program) {
        Command::new(program)
    } else {
        let mut command = Command::new("nix");
        command.arg("shell");
        for package in nix_packages {
            command.arg(format!("nixpkgs#{package}"));
        }
        command.arg("--command").arg(program);
        command
    }
}

fn program_on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| dir.join(program).is_file())
    })
}

fn run_command_streaming_stdout(mut command: Command, label: &str) -> Result<String, String> {
    command.stdout(Stdio::piped());
    command.stderr(Stdio::inherit());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start {label}: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("failed to capture stdout for {label}"))?;
    let reader = BufReader::new(stdout);
    let mut output = String::new();
    for line in reader.lines() {
        let line = line.map_err(|error| format!("failed while reading {label} output: {error}"))?;
        println!("{line}");
        output.push_str(&line);
        output.push('\n');
    }
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for {label}: {error}"))?;
    ensure_success(label, status)?;
    Ok(output)
}

fn ensure_success(label: &str, status: ExitStatus) -> Result<(), String> {
    if status.success() {
        return Ok(());
    }
    Err(format!("{label} failed with status {status}"))
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to read permissions for {}: {error}", path.display()))?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to mark {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

const BENCHKIT_HEAD_SCRIPT: &str = r#"#!/usr/bin/env bash
set -euo pipefail

TOOL_DIR="${BIN_DIR:-/home/mcc/BenchKit/bin}"
TOOL_NAME="${BK_TOOL:-petrivet-mcc}"
INPUT_DIR="${BK_INPUT:-.}"
EXAMINATION="${BK_EXAMINATION:-}"

if [ -z "$EXAMINATION" ]; then
  echo "CANNOT COMPUTE"
  exit 0
fi

TOOL_PATH="$TOOL_DIR/petrivet-mcc"
if [ ! -x "$TOOL_PATH" ]; then
  echo "CANNOT COMPUTE"
  exit 0
fi

export BK_TOOL="$TOOL_NAME"
export BK_EXAMINATION="$EXAMINATION"
export BK_INPUT="$INPUT_DIR"
export BIN_DIR="$TOOL_DIR"

exec "$TOOL_PATH"
"#;
