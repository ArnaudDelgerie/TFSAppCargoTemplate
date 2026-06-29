use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use tauri::{path::BaseDirectory, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const DEV_URL: &str = "http://127.0.0.1:8080";

struct Sidecars {
    server: Option<Child>,
    // The worker is owned by a supervisor thread that respawns it when it exits
    // (time/memory limit or crash), so it lives behind a shared lock.
    worker: Arc<Mutex<Option<Child>>>,
    pid_file: PathBuf,
    shutting_down: Arc<AtomicBool>,
}

impl Sidecars {
    fn stop(&mut self) {
        // Signal the supervisor first so it does not respawn the worker we are
        // about to kill.
        self.shutting_down.store(true, Ordering::SeqCst);
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(mut child) = worker.take() {
                println!("Stopping Messenger worker pid {}", child.id());
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        if let Some(mut child) = self.server.take() {
            println!("Stopping FrankenPHP server pid {}", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_file(&self.pid_file);
    }
}

impl Drop for Sidecars {
    fn drop(&mut self) {
        self.stop();
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let url = if cfg!(debug_assertions) {
                DEV_URL.to_string()
            } else {
                let (sidecars, prod_url) = start_prod_sidecars(app.handle())?;
                app.manage(Mutex::new(sidecars));
                wait_for_healthz(&prod_url)?;
                prod_url
            };

            // Window title follows productName from tauri.conf.json so the
            // title is not a second place to rename when reusing this base.
            let product_name = app
                .config()
                .product_name
                .clone()
                .unwrap_or_else(|| "App".to_string());

            WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(url.parse().expect("valid local backend URL")),
            )
            .title(product_name)
            .inner_size(1100.0, 760.0)
            .min_inner_size(800.0, 560.0)
            .build()?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                if let Some(sidecars) = window.try_state::<Mutex<Sidecars>>() {
                    if let Ok(mut sidecars) = sidecars.lock() {
                        sidecars.stop();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
}

fn start_prod_sidecars(
    app: &tauri::AppHandle,
) -> Result<(Sidecars, String), Box<dyn std::error::Error>> {
    let resource_app = app
        .path()
        .resolve("resources/app", BaseDirectory::Resource)
        .map_err(|error| format!("Cannot resolve bundled Symfony app resource: {error}"))?;
    let public_dir = resource_app.join("public");
    let caddyfile = app
        .path()
        .resolve("resources/Caddyfile.desktop", BaseDirectory::Resource)
        .map_err(|error| format!("Cannot resolve bundled Caddyfile.desktop resource: {error}"))?;
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Cannot resolve app data directory: {error}"))?;
    let pid_file = data_dir.join("sidecars.pids");
    let cache_dir = data_dir.join("cache");
    let build_dir = data_dir.join("build");
    std::fs::create_dir_all(data_dir.join("data"))?;
    cleanup_previous_sidecars(&pid_file)?;
    cleanup_orphaned_installed_sidecars(&caddyfile, &resource_app)?;
    // Wipe both cache and build dir: a stale compiled container/Twig cache can
    // reference old Encore hashes or bundled paths from a previous version.
    let _ = std::fs::remove_dir_all(&cache_dir);
    std::fs::create_dir_all(&cache_dir)?;
    let _ = std::fs::remove_dir_all(&build_dir);
    std::fs::create_dir_all(&build_dir)?;
    std::fs::create_dir_all(data_dir.join("log"))?;

    if !resource_app.join("bin/console").is_file() {
        return Err(format!(
            "Bundled Symfony console not found at {}",
            resource_app.join("bin/console").display()
        )
        .into());
    }
    if !public_dir.join("index.php").is_file() {
        return Err(format!(
            "Bundled Symfony public/index.php not found at {}",
            public_dir.join("index.php").display()
        )
        .into());
    }
    if !caddyfile.is_file() {
        return Err(format!(
            "Bundled Caddyfile.desktop not found at {}",
            caddyfile.display()
        )
        .into());
    }

    let frankenphp = resolve_frankenphp_binary(app)?;
    let database_url = format!("sqlite:///{}", data_dir.join("data/app.db").display());
    let app_dir = path_to_string(&resource_app);
    let public_dir = path_to_string(&public_dir);
    let caddyfile = path_to_string(&caddyfile);
    let port = pick_free_local_port()?;
    let prod_url = format!("http://127.0.0.1:{port}");
    let mercure_url = format!("{prod_url}/.well-known/mercure");
    // The Mercure hub is purely internal: the same loopback process signs and
    // validates JWTs, the hub is in-memory (`transport local`) and ephemeral.
    // Nobody external needs this secret, so we generate a fresh one per launch
    // and never persist it — no hardcoded prod secret to ship.
    let mercure_secret = random_secret_hex()?;
    // APP_SECRET signs CSRF tokens, signed URIs, remember-me cookies, etc.
    // Unlike Mercure it must stay STABLE across launches (anything signed in a
    // previous session must keep validating), so we generate it once per
    // installation and persist it 0600 in app-data — never shipped.
    let app_secret = load_or_create_app_secret(&data_dir)?;

    let envs = [
        ("APP_ENV", "prod".to_string()),
        ("APP_SECRET", app_secret),
        ("APP_DEBUG", "0".to_string()),
        ("APP_PORT", port.to_string()),
        ("APP_ORIGIN", prod_url.clone()),
        ("APP_CACHE_DIR", path_to_string(&cache_dir)),
        ("APP_BUILD_DIR", path_to_string(&build_dir)),
        ("APP_LOG_DIR", path_to_string(&data_dir.join("log"))),
        ("APP_PUBLIC_DIR", public_dir.clone()),
        ("DATABASE_URL", database_url),
        (
            "MESSENGER_TRANSPORT_DSN",
            "doctrine://default?queue_name=async".to_string(),
        ),
        // On loopback the publish URL and the public (browser) URL are identical.
        ("MERCURE_URL", mercure_url.clone()),
        ("MERCURE_PUBLIC_URL", mercure_url),
        // Single secret consumed by both Symfony (signs) and Caddy (validates
        // publisher + subscriber) — see Caddyfile.desktop.
        ("MERCURE_JWT_SECRET", mercure_secret),
    ];

    let server = command_with_env(&frankenphp, &envs)
        .args(["run", "--config", &caddyfile])
        .current_dir(&resource_app)
        .spawn()
        .map_err(|error| {
            format!(
                "Cannot start FrankenPHP server {}: {error}",
                frankenphp.display()
            )
        })?;

    println!("backend listening at {prod_url}");

    // Blocking, before the webview opens. Order is independent: the Messenger
    // table is excluded from ORM migrations (schema_filter), and migrate runs
    // on the persisted app-data DB so a v1->v2 schema change upgrades existing
    // installs instead of breaking on a stale schema.
    run_blocking_console(
        &frankenphp,
        &resource_app,
        &envs,
        &["messenger:setup-transports"],
        "Messenger transport setup",
    )?;
    run_blocking_console(
        &frankenphp,
        &resource_app,
        &envs,
        &["doctrine:migrations:migrate", "--allow-no-migration"],
        "Database migration",
    )?;

    let app_dir = PathBuf::from(app_dir);
    // Owned copy of the environment so the supervisor thread can respawn the
    // worker after the original `envs` array goes out of scope.
    let worker_envs: Vec<(String, String)> = envs
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect();

    let worker = spawn_worker(&frankenphp, &app_dir, &worker_envs).map_err(|error| {
        format!(
            "Cannot start Messenger worker with {}: {error}",
            frankenphp.display()
        )
    })?;

    let server_pid = server.id();
    fs::write(&pid_file, format!("{}\n{}\n", server_pid, worker.id()))?;

    let worker = Arc::new(Mutex::new(Some(worker)));
    let shutting_down = Arc::new(AtomicBool::new(false));
    spawn_worker_supervisor(
        Arc::clone(&worker),
        Arc::clone(&shutting_down),
        frankenphp,
        app_dir,
        worker_envs,
        pid_file.clone(),
        server_pid,
    );

    Ok((
        Sidecars {
            server: Some(server),
            worker,
            pid_file,
            shutting_down,
        },
        prod_url,
    ))
}

/// Spawn a single Messenger worker. `--time-limit`/`--memory-limit` make the
/// worker recycle periodically (healthy for long-lived PHP); the supervisor
/// respawns it afterwards. No `-vvv` in prod: real logs go through Symfony's
/// logger under APP_LOG_DIR.
fn spawn_worker(
    frankenphp: &Path,
    app_dir: &Path,
    envs: &[(String, String)],
) -> std::io::Result<Child> {
    let mut command = Command::new(frankenphp);
    command.env_remove("MERCURE_TRANSPORT_URL");
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .args([
            "php-cli",
            "bin/console",
            "messenger:consume",
            "async",
            "--time-limit=3600",
            "--memory-limit=256M",
            "--env=prod",
            "--no-debug",
        ])
        .current_dir(app_dir)
        .spawn()
}

/// Watch the worker child and respawn it whenever it exits, unless the app is
/// shutting down. Without this, the worker would stop consuming the queue after
/// its first time/memory limit and async jobs would silently never complete.
fn spawn_worker_supervisor(
    worker: Arc<Mutex<Option<Child>>>,
    shutting_down: Arc<AtomicBool>,
    frankenphp: PathBuf,
    app_dir: PathBuf,
    envs: Vec<(String, String)>,
    pid_file: PathBuf,
    server_pid: u32,
) {
    thread::spawn(move || loop {
        if shutting_down.load(Ordering::SeqCst) {
            return;
        }

        // Poll with try_wait so we never hold the lock while blocking — stop()
        // must always be able to grab it to kill the worker.
        let exited = {
            let Ok(mut guard) = worker.lock() else {
                return;
            };
            match guard.as_mut() {
                Some(child) => matches!(child.try_wait(), Ok(Some(_))),
                None => return,
            }
        };

        if exited {
            if shutting_down.load(Ordering::SeqCst) {
                return;
            }
            match spawn_worker(&frankenphp, &app_dir, &envs) {
                Ok(child) => {
                    let worker_pid = child.id();
                    println!("Restarted Messenger worker pid {worker_pid}");
                    let _ = fs::write(&pid_file, format!("{server_pid}\n{worker_pid}\n"));
                    if let Ok(mut guard) = worker.lock() {
                        *guard = Some(child);
                    }
                }
                Err(error) => {
                    eprintln!("Cannot restart Messenger worker: {error}");
                    return;
                }
            }
        }

        thread::sleep(Duration::from_millis(1000));
    });
}

fn cleanup_previous_sidecars(pid_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let Ok(contents) = fs::read_to_string(pid_file) else {
        return Ok(());
    };

    for line in contents.lines() {
        let Ok(pid) = line.trim().parse::<u32>() else {
            continue;
        };

        if process_command_contains(pid, "frankenphp") {
            println!("Stopping stale sidecar pid {pid}");
            let _ = Command::new("kill").arg(pid.to_string()).status();
            wait_for_process_exit(pid, Duration::from_secs(2));
            if process_command_contains(pid, "frankenphp") {
                let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
            }
        }
    }

    let _ = fs::remove_file(pid_file);
    Ok(())
}

fn cleanup_orphaned_installed_sidecars(
    caddyfile: &Path,
    app_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Derive the match criteria from the resolved install paths so renaming the
    // app (productName) needs no edit here — the old hardcoded path was a footgun
    // when reusing this base.
    let caddyfile = caddyfile.to_string_lossy();
    let app_dir = app_dir.canonicalize().unwrap_or_else(|_| app_dir.to_path_buf());

    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(pid) = file_name.parse::<u32>() else {
            continue;
        };
        let cmdline = process_command_line(pid);
        if is_app_sidecar(&cmdline, &caddyfile, pid, &app_dir) {
            println!("Stopping orphaned sidecar pid {pid}");
            let _ = Command::new("kill").arg(pid.to_string()).status();
            wait_for_process_exit(pid, Duration::from_secs(2));
            if process_command_contains(pid, "frankenphp") {
                let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
            }
        }
    }

    Ok(())
}

/// True if `pid` is a leftover sidecar of THIS app after a crash. The server is
/// matched by its install-specific Caddyfile path; the worker by its Messenger
/// command scoped to this app's working directory, so sibling apps built on the
/// same base never kill each other's workers.
fn is_app_sidecar(cmdline: &str, caddyfile: &str, pid: u32, app_dir: &Path) -> bool {
    if cmdline.contains(caddyfile) {
        return true;
    }

    let is_worker = cmdline.contains("bin/console messenger:consume async")
        && cmdline.contains("--env=prod")
        && cmdline.contains("--no-debug");

    is_worker && process_cwd(pid).as_deref() == Some(app_dir)
}

fn process_cwd(pid: u32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

fn process_command_contains(pid: u32, needle: &str) -> bool {
    process_command_line(pid).contains(needle)
}

fn process_command_line(pid: u32) -> String {
    fs::read_to_string(format!("/proc/{pid}/cmdline"))
        .map(|cmdline| cmdline.replace('\0', " "))
        .unwrap_or_default()
}

fn wait_for_process_exit(pid: u32, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !Path::new(&format!("/proc/{pid}")).exists() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Generate a random secret as 64 hex chars (32 bytes of entropy). Stays well
/// above the 256-bit floor `lcobucci/jwt` (HS256) imposes on Mercure keys.
fn random_secret_hex() -> Result<String, Box<dyn std::error::Error>> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| format!("Cannot generate secret: {error}"))?;
    let mut secret = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        secret.push_str(&format!("{byte:02x}"));
    }
    Ok(secret)
}

/// Read the persisted APP_SECRET from app-data, or generate and persist one on
/// first run. Stored 0600 so only the current user can read it. Stable across
/// launches so anything Symfony signs keeps validating after a restart.
fn load_or_create_app_secret(data_dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let secret_file = data_dir.join("data/app.secret");

    if let Ok(existing) = fs::read_to_string(&secret_file) {
        let existing = existing.trim();
        if !existing.is_empty() {
            return Ok(existing.to_string());
        }
    }

    let secret = random_secret_hex()?;
    fs::write(&secret_file, &secret)?;
    restrict_to_owner(&secret_file)?;
    Ok(secret)
}

/// Restrict a file to owner read/write only (0600). No-op on non-Unix targets.
#[cfg(unix)]
fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn pick_free_local_port() -> Result<u16, Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

fn wait_for_healthz(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{base_url}/healthz");
    let deadline = Instant::now() + Duration::from_secs(20);

    while Instant::now() < deadline {
        if let Ok(response) = ureq::get(&url).call() {
            if response.status() == 200 {
                return Ok(());
            }
        }

        thread::sleep(Duration::from_millis(250));
    }

    Err(format!("backend did not become healthy at {url}").into())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn command_with_env(binary: &Path, envs: &[(&str, String)]) -> Command {
    let mut command = Command::new(binary);
    command.env_remove("MERCURE_TRANSPORT_URL");
    for (key, value) in envs {
        command.env(key, value);
    }
    command
}

/// Run a Symfony console command to completion and fail hard if it does not
/// succeed. Used for the blocking pre-webview setup steps (transports, schema
/// migrations). `--no-interaction`, `--env=prod` and `--no-debug` are appended.
fn run_blocking_console(
    frankenphp: &Path,
    resource_app: &Path,
    envs: &[(&str, String)],
    console_args: &[&str],
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["php-cli", "bin/console"];
    args.extend_from_slice(console_args);
    args.extend_from_slice(&["--no-interaction", "--env=prod", "--no-debug"]);

    let status = command_with_env(frankenphp, envs)
        .args(&args)
        .current_dir(resource_app)
        .status()
        .map_err(|error| format!("Cannot run {label} with {}: {error}", frankenphp.display()))?;

    if !status.success() {
        return Err(format!("{label} failed with status {status}").into());
    }

    Ok(())
}

fn resolve_frankenphp_binary(
    app: &tauri::AppHandle,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Bundled location: under the app's own resource dir
    // (/usr/lib/<ProductName>/resources/frankenphp), so the sidecar is namespaced
    // per app and never collides with another TFS app on /usr/bin/frankenphp.
    if let Ok(resource) = app
        .path()
        .resolve("resources/frankenphp", BaseDirectory::Resource)
    {
        if resource.is_file() {
            return Ok(resource);
        }
    }

    // Fallbacks: next to the executable, then a legacy system-wide install.
    let current_exe = std::env::current_exe()?;
    let exe_dir = current_exe
        .parent()
        .ok_or("Cannot resolve current executable directory")?;
    let next_to_exe = exe_dir.join("frankenphp");
    if next_to_exe.is_file() {
        return Ok(next_to_exe);
    }

    let usr_bin = PathBuf::from("/usr/bin/frankenphp");
    if usr_bin.is_file() {
        return Ok(usr_bin);
    }

    Err(format!(
        "FrankenPHP sidecar not found. Tried bundled resources/frankenphp, {} and {}",
        next_to_exe.display(),
        usr_bin.display()
    ).into())
}
