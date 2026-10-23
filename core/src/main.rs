use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::os::fd::AsRawFd;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
mod engine;
mod launcher;
mod removal;

fn local<T>(ru: T, en: T) -> T {
    if std::env::var("VELA_LANGUAGE").as_deref() == Ok("en") { en } else { ru }
}

type Result<T> = std::result::Result<T, String>;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Default, Serialize, Deserialize)]
struct State {
    #[serde(default = "schema")]
    schema: u32,
    #[serde(default)]
    engines: Vec<Engine>,
    #[serde(default)]
    bottles: Vec<Bottle>,
    #[serde(default)]
    deleted_bottles: Vec<Bottle>,
}
fn schema() -> u32 { 1 }

#[derive(Clone, Default, Serialize, Deserialize)]
struct Engine {
    id: String, name: String, wine: String, version: String,
    #[serde(default)] server: String,
    #[serde(default)] runtime_root: Option<PathBuf>,
    #[serde(default)] architecture: String,
    #[serde(default)] backend: String,
    #[serde(default)] supports_x86_32: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Bottle {
    id: String,
    name: String,
    engine_id: String,
    status: String,
    #[serde(default)]
    environment: BTreeMap<String, String>,
    #[serde(default)]
    shortcuts: Vec<Shortcut>,
    #[serde(default)]
    last_log: Option<String>,
    #[serde(default)]
    dpi: Option<u32>,
    #[serde(default)]
    installation: Option<launcher::Installation>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Shortcut { id: String, name: String, executable: String, arguments: Vec<String>, directory: Option<String> }

#[derive(Deserialize)]
struct Request {
    op: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    engine_id: String,
    #[serde(default)]
    arguments: Vec<String>,
    directory: Option<String>,
    #[serde(default)]
    environment: BTreeMap<String, String>,
    #[serde(default)]
    shortcut_id: String,
    #[serde(default)]
    tool: String,
    dpi: Option<u32>,
    #[serde(default)]
    launcher_id: String,
}

fn uid() -> String {
    format!("{:x}-{:x}-{:x}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(), std::process::id(), SEQUENCE.fetch_add(1, Ordering::Relaxed))
}

struct Store { root: PathBuf }
struct Lock(PathBuf);
impl Drop for Lock { fn drop(&mut self) { let _ = fs::remove_file(&self.0); } }
struct OperationLock { _file: File }
unsafe extern "C" { fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int; }

impl Store {
    fn new(root: PathBuf) -> Result<Self> {
        if !root.is_absolute() { return Err(local("Папка данных должна иметь абсолютный путь", "The data folder must use an absolute path").into()); }
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        for dir in ["prefixes", "logs", "locks"] { fs::create_dir_all(root.join(dir)).map_err(|e| e.to_string())?; }
        Ok(Self { root })
    }
    fn lock(&self) -> Result<Lock> {
        let path = self.root.join("state.lock");
        let start = Instant::now();
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut f) => { let _ = writeln!(f, "{}", std::process::id()); return Ok(Lock(path)); }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && start.elapsed() < Duration::from_secs(5) => std::thread::sleep(Duration::from_millis(25)),
                Err(e) => return Err(local(format!("Не удалось заблокировать state.json: {e}. Если Vela аварийно завершилась, проверьте state.lock."), format!("Could not lock state.json: {e}. If Vela crashed, check state.lock."))),
            }
        }
    }
    fn operation_lock(&self, id: &str) -> Result<OperationLock> {
        self.prefix(id)?;
        let path = self.root.join("locks").join(format!("{id}.lock"));
        let mut file = OpenOptions::new().write(true).create(true).truncate(false).open(&path).map_err(|e| e.to_string())?;
        // LOCK_EX | LOCK_NB: the kernel releases this lock even if the core is
        // killed or crashes. The open file remains owned by OperationLock.
        if unsafe { flock(file.as_raw_fd(), 2 | 4) } != 0 {
            return Err(local("Окружение занято другой операцией. Дождитесь её завершения.", "This environment is busy. Wait for the current operation to finish.").into());
        }
        file.set_len(0).map_err(|e| e.to_string())?;
        writeln!(file, "{}", std::process::id()).map_err(|e| e.to_string())?;
        Ok(OperationLock { _file: file })
    }
    fn read(&self) -> Result<State> {
        let file = self.root.join("state.json");
        if !file.exists() { return Ok(State { schema: 1, ..State::default() }); }
        let mut s: State = serde_json::from_slice(&fs::read(file).map_err(|e| e.to_string())?).map_err(|e| local(format!("Не удалось прочитать данные: {e}"), format!("Could not read library data: {e}")))?;
        if s.schema != 1 { return Err(local("Эта версия Vela не поддерживает формат данных", "This Vela version does not support the library format").into()); }
        for e in &mut s.engines {
                if let Ok(location) = engine::locate(&e.wine) {
                    e.server = location.server.to_string_lossy().into_owned();
                    e.runtime_root = location.root;
                    e.architecture = location.architecture;
                    e.backend = location.backend;
                    e.supports_x86_32 = location.supports_x86_32;
                }
        }
        Ok(s)
    }
    fn write(&self, s: &State) -> Result<()> {
        let temp = self.root.join(format!("state-{}.tmp", uid()));
        let mut f = File::create(&temp).map_err(|e| e.to_string())?;
        f.write_all(&serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temp, self.root.join("state.json")).map_err(|e| e.to_string())
    }
    fn update<T>(&self, f: impl FnOnce(&mut State) -> Result<T>) -> Result<T> {
        let _lock = self.lock()?;
        let mut s = self.read()?;
        let result = f(&mut s)?;
        self.write(&s)?;
        Ok(result)
    }
    fn prefix(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') { return Err(local("Некорректный идентификатор окружения", "Invalid environment ID").into()); }
        let p = self.root.join("prefixes").join(id);
        if self.root.join("prefixes").is_symlink() || p.is_symlink() { return Err(local("Папка окружения не должна быть символической ссылкой", "The environment folder must not be a symbolic link").into()); }
        Ok(p)
    }
    fn pair(&self, id: &str) -> Result<(Bottle, Engine)> {
        let s = self.read()?;
        let b = s.bottles.iter().find(|b| b.id == id).ok_or(local("Окружение не найдено", "Environment not found"))?.clone();
        let e = s.engines.iter().find(|e| e.id == b.engine_id).ok_or(local("Движок не найден", "Engine not found"))?.clone();
        Ok((b, e))
    }
    fn log(&self, b: &Bottle, label: &str) -> Result<(PathBuf, File)> {
        let name = format!("{}-{}.log", b.id, uid());
        let path = self.root.join("logs").join(&name);
        let mut file = File::create(&path).map_err(|e| e.to_string())?;
        writeln!(file, "Vela · {label}\nEnvironment: {}\n", b.name).map_err(|e| e.to_string())?;
        self.update(|s| {
            s.bottles.iter_mut().chain(s.deleted_bottles.iter_mut()).find(|x| x.id == b.id)
                .ok_or(local("Окружение не найдено", "Environment not found"))?.last_log = Some(name); Ok(())
        })?;
        Ok((path, file))
    }
}

fn bottle_mut<'a>(s: &'a mut State, id: &str) -> Result<&'a mut Bottle> { s.bottles.iter_mut().find(|b| b.id == id).ok_or_else(|| local("Окружение не найдено", "Environment not found").into()) }
fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) { return Err(local("Введите название длиной от 1 до 80 символов", "Enter a name between 1 and 80 characters").into()); }
    Ok(name.to_string())
}
fn valid_environment(env: &BTreeMap<String, String>) -> Result<()> {
    for (key, value) in env {
        if key.is_empty() || key.as_bytes()[0].is_ascii_digit() || !key.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            return Err(local(format!("Недопустимое имя переменной: {key}"), format!("Invalid variable name: {key}")));
        }
        if ["WINEPREFIX", "WINEARCH", "WINESERVER", "WINELOADER", "HOME", "PATH", "TMPDIR"].contains(&key.as_str()) || key.starts_with("DYLD_") || key.starts_with("LD_") || key.starts_with("VELA_") {
            return Err(local(format!("Переменной {key} управляет Vela"), format!("Vela manages the {key} variable")));
        }
        if value.contains('\0') { return Err(local("Переменная содержит нулевой байт", "A variable contains a null byte").into()); }
    }
    Ok(())
}
fn absolute_file(path: &str) -> Result<PathBuf> {
    let p = PathBuf::from(path);
    if !p.is_absolute() || !p.is_file() { return Err(local(format!("Файл не найден: {path}"), format!("File not found: {path}"))); }
    fs::canonicalize(p).map_err(|e| e.to_string())
}
fn base_command(wine: &str) -> Command {
    // vela-core itself is native ARM64. Direct spawning preserves runtime-specific
    // DYLD paths; an intervening platform launcher can discard those variables.
    let mut c = Command::new(wine);
    c.env_clear();
    for (key, value) in std::env::vars_os() {
        let text = key.to_string_lossy();
        if ["HOME", "USER", "LOGNAME", "SHELL", "LANG", "TMPDIR", "DISPLAY", "XAUTHORITY"].contains(&text.as_ref()) || text.starts_with("LC_") {
            c.env(key, value);
        }
    }
    if let Some(parent) = Path::new(wine).parent() {
        let mut paths = vec![parent.to_path_buf()];
        if let Some(path) = std::env::var_os("PATH") { paths.extend(std::env::split_paths(&path)); }
        if let Ok(path) = std::env::join_paths(paths) { c.env("PATH", path); }
    }
    c.stdin(Stdio::null());
    c
}
fn runtime_command(e: &Engine, executable: &str) -> Result<Command> {
    let native = e.architecture == "arm64";
    if native && !engine::file_architectures(Path::new(executable))?.iter().any(|a| a == "arm64") {
        return Err(local("Файлы ARM64-движка изменились или были заменены на Intel. Подключите проверенную нативную сборку заново.", "ARM64 engine files have changed or were replaced with Intel binaries. Register a verified native build again.").into());
    }
    let mut c = base_command(executable);
    c.env("WINELOADER", &e.wine);
    if !e.server.is_empty() { c.env("WINESERVER", &e.server); }
    if e.backend == "arm64-fex" {
        let root = e.runtime_root.as_ref().ok_or(local("Не найдена папка ARM64/FEX runtime", "ARM64/FEX runtime folder not found"))?;
        let paths = [root.join("mesa-zink/lib"), root.join("ext/lib")];
        let paths: Vec<_> = paths.iter().filter(|p| p.is_dir()).collect();
        if !paths.is_empty() {
            c.env("DYLD_FALLBACK_LIBRARY_PATH", std::env::join_paths(paths).map_err(|e| e.to_string())?);
        }
        c.env("FEX_DISKCACHE", "1").env("HADRON_DISABLE_LSTEAMCLIENT", "1");
    }
    Ok(c)
}
fn wine_command(store: &Store, b: &Bottle, e: &Engine) -> Result<Command> {
    valid_environment(&b.environment)?;
    let mut c = runtime_command(e, &e.wine)?;
    c.env("WINEDEBUG", "fixme-all").env("WINEDLLOVERRIDES", "winemenubuilder.exe=d").envs(&b.environment);
    c.env("WINEPREFIX", store.prefix(&b.id)?).env("WINEARCH", "win64");
    Ok(c)
}
fn wait_bounded(child: &mut Child, timeout: Duration) -> Result<std::process::ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? { return Ok(status); }
        if start.elapsed() >= timeout {
            let _ = child.kill(); let _ = child.wait();
            return Err(local("Истекло время ожидания. Откройте журнал окружения; фоновые процессы Wine можно остановить кнопкой «Остановить».", "The operation timed out. Open the environment log; use Stop to stop its background Wine processes.").into());
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}
fn probe(store: &Store, e: &Engine) -> Result<String> {
    let output = store.root.join(format!("probe-{}.tmp", uid()));
    let file = File::create(&output).map_err(|e| e.to_string())?;
    let mut c = runtime_command(e, &e.wine)?;
    c.arg("--version").stdout(file.try_clone().map_err(|e| e.to_string())?).stderr(file);
    let result = (|| {
        let mut child = c.spawn().map_err(|e| local(format!("Не удалось запустить Wine: {e}"), format!("Could not start Wine: {e}")))?;
        let status = wait_bounded(&mut child, Duration::from_secs(15))?;
        let text = fs::read_to_string(&output).map_err(|e| e.to_string())?;
        if !status.success() {
            let hint = if e.architecture == "arm64" { local("Проверьте требования движка к macOS и его подпись.", "Check the engine's macOS requirements and code signature.") } else { local("Для сборок x86_64 на Apple Silicon нужна Rosetta.", "x86_64 builds require Rosetta on Apple Silicon.") };
            return Err(local(format!("Wine не запускается: {text}. {hint}"), format!("Wine could not start: {text}. {hint}")));
        }
        let version = text.lines().find(|l| l.to_lowercase().contains("wine-")).ok_or_else(|| local(format!("Не удалось определить версию Wine: {text}"), format!("Could not determine the Wine version: {text}")))?;
        Ok(version.trim().to_string())
    })();
    let _ = fs::remove_file(output);
    result
}
fn launch(store: &Store, b: &Bottle, e: &Engine, args: &[String], directory: Option<&str>, label: &str) -> Result<Value> {
    let _operation = store.operation_lock(&b.id)?;
    let (current, _) = store.pair(&b.id)?;
    let b = &current;
    if b.status != "ready" { return Err(local("Окружение не готово. Повторите инициализацию.", "The environment is not ready. Retry initialization.").into()); }
    if args.iter().any(|a| a.contains('\0')) { return Err(local("Аргумент содержит нулевой байт", "An argument contains a null byte").into()); }
    let (path, log) = store.log(b, label)?;
    let mut c = wine_command(store, b, e)?;
    c.args(args).stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log);
    let cwd = directory.map(PathBuf::from).unwrap_or(store.prefix(&b.id)?.join("drive_c"));
    if !cwd.is_absolute() || !cwd.is_dir() { return Err(local("Рабочая папка не найдена", "Working folder not found").into()); }
    c.current_dir(cwd);
    // Wine owns the Windows process tree. Closing this CLI must not close the app.
    let child = c.spawn().map_err(|e| local(format!("Не удалось запустить: {e}"), format!("Could not launch: {e}")))?;
    Ok(json!({"pid": child.id(), "log": path, "message": local("Запуск передан Wine. Результат запуска смотрите в журнале.", "Launch request sent to Wine. Check the log for the result.")}))
}
fn initialize(store: &Store, id: &str) -> Result<Value> {
    let _operation = store.operation_lock(id)?;
    let (b, e) = store.pair(id)?;
    let result = (|| {
        let (path, file) = store.log(&b, "wineboot --init")?;
        let mut c = wine_command(store, &b, &e)?;
        if e.backend == "arm64-fex" {
            // Optional Mono/Gecko installers can wait on a hidden first-run dialog.
            // Suppress them only during initialization, never for application launches.
            let overrides = b.environment.get("WINEDLLOVERRIDES").map(String::as_str).unwrap_or("winemenubuilder.exe=d");
            c.env("WINEDLLOVERRIDES", format!("{overrides};mscoree,mshtml=d"));
        }
        c.args(["wineboot", "--init"]).stdout(file.try_clone().map_err(|e| e.to_string())?).stderr(file);
        let mut child = c.spawn().map_err(|e| e.to_string())?;
        let status = wait_bounded(&mut child, Duration::from_secs(120))?;
        if !status.success() { return Err(local(format!("Wine завершил настройку с ошибкой ({status}). Журнал: {}", path.display()), format!("Wine initialization failed ({status}). Log: {}", path.display()))); }
        // wineserver saves registry files asynchronously after wineboot exits.
        let registry = store.prefix(id)?.join("system.reg");
        let started = Instant::now();
        while !registry.exists() && started.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(100));
        }
        if !registry.exists() { return Err(local(format!("Wine ещё не сохранил реестр окружения. Повторите инициализацию. Журнал: {}", path.display()), format!("Wine has not saved the environment registry yet. Retry initialization. Log: {}", path.display()))); }
        if let Some(dpi) = b.dpi { apply_dpi(store, &b, &e, dpi)?; }
        Ok(json!({"id":id, "log":path}))
    })();
    store.update(|s| { bottle_mut(s, id)?.status = if result.is_ok() { "ready" } else { "failed" }.into(); Ok(()) })?;
    result
}
fn validate_dpi(dpi: u32) -> Result<()> {
    if ![96, 120, 144, 168, 192, 240, 288].contains(&dpi) { return Err(local("Выберите масштаб от 100% до 300% из списка", "Choose a scale between 100% and 300% from the list").into()); }
    Ok(())
}
fn apply_dpi(store: &Store, b: &Bottle, e: &Engine, dpi: u32) -> Result<Value> {
    validate_dpi(dpi)?;
    let (path, file) = store.log(b, local("Масштаб Windows", "Windows scale"))?;
    let mut c = wine_command(store, b, e)?;
    c.args(["reg.exe", "add", "HKCU\\Control Panel\\Desktop", "/v", "LogPixels", "/t", "REG_DWORD", "/d", &dpi.to_string(), "/f"])
        .stdout(file.try_clone().map_err(|e| e.to_string())?).stderr(file);
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    let status = wait_bounded(&mut child, Duration::from_secs(20))?;
    if !status.success() { return Err(local(format!("Не удалось изменить масштаб Windows. Журнал: {}", path.display()), format!("Could not change Windows scale. Log: {}", path.display()))); }
    store.update(|s| { bottle_mut(s, &b.id)?.dpi = Some(dpi); Ok(()) })?;
    Ok(json!({"dpi":dpi, "message":local("Масштаб сохранён. Перезапустите Windows-программы, чтобы применить его.", "Scale saved. Restart Windows applications to apply it.")}))
}
fn dispatch(store: &Store, r: Request) -> Result<Value> {
    match r.op.as_str() {
        "list" => { launcher::reconcile(store)?; Ok(json!({"state": store.read()?, "root": store.root})) },
        "catalog" => Ok(json!({"launchers": launcher::catalog()?})),
        "install_launcher" => launcher::install(store, &r),
        "retry_launcher" => launcher::retry(store, &r.id),
        "delete_environment" => removal::trash(store, &r.id),
        "restore_environment" => removal::restore(store, &r.id),
        "purge_environment" => removal::purge(store, &r.id),
        "add_engine" => {
            let location = engine::locate(&r.path)?;
            let mut e = Engine { id: uid(), name: String::new(), wine: location.wine.to_string_lossy().into_owned(), version: String::new(),
                server: location.server.to_string_lossy().into_owned(), runtime_root: location.root, architecture: location.architecture, backend: location.backend, supports_x86_32: location.supports_x86_32 };
            let version = probe(store, &e)?;
            let name = if r.name.trim().is_empty() { version.clone() } else { clean_name(&r.name)? };
            e.name = name; e.version = version;
            store.update(|s| {
                if s.engines.iter().any(|x| x.wine == e.wine) { return Err(local("Этот движок уже добавлен", "This engine is already registered").into()); }
                s.engines.push(e.clone()); Ok(())
            })?;
            Ok(json!(e))
        }
        "create" => {
            let name = clean_name(&r.name)?;
            if let Some(dpi) = r.dpi { validate_dpi(dpi)?; }
            let b = Bottle { id: uid(), name, engine_id: r.engine_id, status: "initializing".into(), environment: BTreeMap::new(), shortcuts: vec![], last_log: None, dpi:r.dpi, installation:None };
            store.update(|s| {
                if !s.engines.iter().any(|e| e.id == b.engine_id) { return Err(local("Сначала добавьте движок Wine", "Add a Wine engine first").into()); }
                fs::create_dir(store.prefix(&b.id)?).map_err(|e| e.to_string())?;
                s.bottles.push(b.clone()); Ok(())
            })?;
            initialize(store, &b.id)
        }
        "initialize" => {
            let (b, _) = store.pair(&r.id)?;
            if b.status == "ready" { return Err(local("Окружение уже готово", "The environment is already ready").into()); }
            initialize(store, &r.id)
        }
        "set_dpi" => {
            let _operation = store.operation_lock(&r.id)?;
            let (b, e) = store.pair(&r.id)?;
            if b.status != "ready" { return Err(local("Сначала завершите создание окружения", "Finish creating the environment first").into()); }
            apply_dpi(store, &b, &e, r.dpi.ok_or(local("Выберите масштаб Windows", "Choose a Windows scale"))?)
        }
        "save_settings" => {
            let name = clean_name(&r.name)?;
            valid_environment(&r.environment)?;
            store.update(|s| { let b = bottle_mut(s, &r.id)?; b.name = name; b.environment = r.environment; Ok(()) })?;
            Ok(json!({}))
        }
        "add_shortcut" => {
            let name = clean_name(&r.name)?;
            let file = absolute_file(&r.path)?;
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if !["exe", "msi", "bat", "cmd"].contains(&ext.as_str()) { return Err(local("Выберите EXE, MSI, BAT или CMD", "Choose an EXE, MSI, BAT, or CMD file").into()); }
            if let Some(dir) = &r.directory { if !Path::new(dir).is_absolute() || !Path::new(dir).is_dir() { return Err(local("Рабочая папка не найдена", "Working folder not found").into()); } }
            let shortcut = Shortcut { id: uid(), name, executable: file.to_string_lossy().into_owned(), arguments: r.arguments, directory: r.directory };
            store.update(|s| { bottle_mut(s, &r.id)?.shortcuts.push(shortcut.clone()); Ok(()) })?;
            Ok(json!(shortcut))
        }
        "remove_shortcut" => { store.update(|s| { bottle_mut(s, &r.id)?.shortcuts.retain(|s| s.id != r.shortcut_id); Ok(()) })?; Ok(json!({})) }
        "launch" | "run_shortcut" => {
            let (b, e) = store.pair(&r.id)?;
            let (file, arguments, directory) = if r.op == "run_shortcut" {
                let shortcut = b.shortcuts.iter().find(|s| s.id == r.shortcut_id).ok_or(local("Ярлык не найден", "Shortcut not found"))?;
                (shortcut.executable.clone(), shortcut.arguments.clone(), shortcut.directory.clone())
            } else { (r.path, r.arguments, r.directory) };
            let path = absolute_file(&file)?;
            if e.supports_x86_32 == Some(false) && engine::pe_machine(&path) == Some(0x014c) {
                return Err(local("Это 32-битная Windows-программа. Этот нативный движок поддерживает только 64-битные программы. Выберите 64-битную версию программы; для нативного запуска 32-битных программ нужна отдельная сборка с разрешением Apple.", "This is a 32-bit Windows application. This native engine supports 64-bit programs only. Choose a 64-bit application; native 32-bit support requires a separate runtime with Apple's permission.").into());
            }
            let args = executable_args(&path, &arguments)?;
            let cwd = directory.or_else(|| path.parent().map(|p| p.to_string_lossy().into_owned()));
            launch(store, &b, &e, &args, cwd.as_deref(), &file)
        }
        "tool" => {
            let (b, e) = store.pair(&r.id)?;
            let args: Vec<String> = match r.tool.as_str() {
                "winecfg" => vec!["winecfg".into()],
                "regedit" => vec!["regedit".into()],
                "uninstaller" => vec!["uninstaller".into()],
                "cmd" => vec!["wineconsole".into(), "cmd".into()],
                _ => return Err(local("Неизвестный инструмент Wine", "Unknown Wine tool").into()),
            };
            launch(store, &b, &e, &args, None, &r.tool)
        }
        "stop" => {
            let (b, e) = store.pair(&r.id)?;
            let _operation = store.operation_lock(&r.id)?;
            stop_environment(store, &b, &e)
        }
        "read_log" => {
            let (b, _) = store.pair(&r.id)?;
            let Some(name) = b.last_log else { return Ok(json!({"text":local("Журнал появится после первого запуска.", "A log will appear after the first launch.")})); };
            if Path::new(&name).components().count() != 1 || !name.ends_with(".log") { return Err(local("Некорректный путь журнала", "Invalid log path").into()); }
            let path = store.root.join("logs").join(name);
            let mut f = File::open(&path).map_err(|e| e.to_string())?;
            let len = f.metadata().map_err(|e| e.to_string())?.len();
            f.seek(SeekFrom::Start(len.saturating_sub(128 * 1024))).map_err(|e| e.to_string())?;
            let mut data = vec![]; f.take(128 * 1024).read_to_end(&mut data).map_err(|e| e.to_string())?;
            Ok(json!({"text":String::from_utf8_lossy(&data), "path":path}))
        }
        _ => Err(local("Неизвестная операция", "Unknown operation").into()),
    }
}
fn stop_environment(store: &Store, b: &Bottle, e: &Engine) -> Result<Value> {
    let server = if e.server.is_empty() { Path::new(&e.wine).parent().ok_or(local("Не найдена папка Wine", "Wine folder not found"))?.join("wineserver") } else { PathBuf::from(&e.server) };
    let (path, log) = store.log(&b, "wineserver -k")?;
    let output_start = log.metadata().map_err(|e| e.to_string())?.len();
    let mut c = wine_command(store, &b, &e)?;
    let env: Vec<_> = c.get_envs().map(|(k, v)| (k.to_owned(), v.map(|s| s.to_owned()))).collect();
    c = runtime_command(&e, &server.to_string_lossy())?;
    for (key, value) in env { if let Some(v) = value { c.env(key, v); } else { c.env_remove(key); } }
    let mut child = c.arg("-k").stdin(Stdio::null()).stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log).spawn().map_err(|e| e.to_string())?;
    let status = wait_bounded(&mut child, Duration::from_secs(15))?;
    if status.code() == Some(1) {
        let mut file = File::open(&path).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(output_start)).map_err(|e| e.to_string())?;
        let mut body = Vec::new(); file.take(16384).read_to_end(&mut body).map_err(|e| e.to_string())?;
        // Upstream wineserver returns 1, without output, when no server owns the prefix.
        if body.is_empty() { return Ok(json!({"message":local("Окружение уже остановлено", "The environment is already stopped")})); }
    }
    if !status.success() { return Err(local(format!("Не удалось остановить Wine. Журнал: {}", path.display()), format!("Could not stop Wine. Log: {}", path.display()))); }
    Ok(json!({"message":local("Команда остановки отправлена только этому окружению", "Stop request sent only to this environment")}))
}
fn executable_args(path: &Path, arguments: &[String]) -> Result<Vec<String>> {
    let file = path.to_string_lossy().into_owned();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let mut args = match ext.as_str() {
        "exe" => vec![file],
        "msi" => vec!["msiexec".into(), "/i".into(), file],
        "bat" | "cmd" => vec!["cmd".into(), "/c".into(), file],
        _ => return Err(local("Поддерживаются EXE, MSI, BAT и CMD", "EXE, MSI, BAT, and CMD files are supported").into()),
    };
    args.extend_from_slice(arguments); Ok(args)
}
fn main() {
    let result = (|| {
        let mut input = String::new();
        std::io::stdin().lock().take(4 * 1024 * 1024).read_line(&mut input).map_err(|e| e.to_string())?;
        let r: Request = serde_json::from_str(&input).map_err(|e| local(format!("Некорректный запрос: {e}"), format!("Invalid request: {e}")))?;
        let root = std::env::var_os("VELA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support/Vela"))).ok_or(local("Не задан путь данных", "No data folder was specified"))?;
        dispatch(&Store::new(root)?, r)
    })();
    match result {
        Ok(data) => println!("{}", json!({"ok":true,"data":data})),
        Err(error) => { println!("{}", json!({"ok":false,"error":error})); std::process::exit(1); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn store() -> Store { Store::new(std::env::temp_dir().join(format!("vela-test-{}", uid()))).unwrap() }
    fn req(v: Value) -> Request { serde_json::from_value(v).unwrap() }
    #[test] fn path_traversal_is_rejected() {
        let s = store();
        for id in ["../other", "", "/tmp", "abcd/ef"] { assert!(s.prefix(id).is_err()); }
        fs::remove_dir_all(s.root).unwrap();
    }
    #[test] fn isolation_variables_cannot_be_overridden() {
        for key in ["WINEPREFIX", "WINEARCH", "WINESERVER", "PATH", "DYLD_INSERT_LIBRARIES", "LD_PRELOAD"] {
            assert!(valid_environment(&BTreeMap::from([(key.into(), "bad".into())])).is_err());
        }
        assert!(valid_environment(&BTreeMap::from([("WINEDLLOVERRIDES".into(), "d3d11=n,b".into())])).is_ok());
    }
    #[test] fn arguments_keep_spaces_and_shell_characters_literal() {
        let args = executable_args(Path::new("/tmp/My Setup.MSI"), &["$HOME; touch /tmp/bad".into()]).unwrap();
        assert_eq!(args, vec!["msiexec", "/i", "/tmp/My Setup.MSI", "$HOME; touch /tmp/bad"]);
    }
    #[test] fn corrupt_or_future_state_is_not_overwritten() {
        let s = store();
        for text in ["invalid", "{\"schema\":2,\"engines\":[],\"bottles\":[]}"] {
            fs::write(s.root.join("state.json"), text).unwrap();
            assert!(s.update(|_| Ok(())).is_err());
            assert_eq!(fs::read_to_string(s.root.join("state.json")).unwrap(), text);
        }
        fs::remove_dir_all(s.root).unwrap();
    }
    #[test] fn concurrent_updates_do_not_lose_changes() {
        let s = store(); let root = s.root.clone();
        let threads: Vec<_> = (0..8).map(|n| { let root = root.clone(); std::thread::spawn(move || {
            let s = Store::new(root).unwrap();
            s.update(|state| { state.engines.push(Engine {id:uid(),name:n.to_string(),wine:"/test/wine".into(),version:"test".into(), ..Engine::default()}); Ok(()) }).unwrap();
        }) }).collect();
        for t in threads { t.join().unwrap(); }
        assert_eq!(s.read().unwrap().engines.len(), 8);
        fs::remove_dir_all(s.root).unwrap();
    }
    #[test] fn fake_runner_lifecycle_is_scoped_and_preserves_arguments() {
        let s = store(); let bin = s.root.join("Engine with spaces/bin"); fs::create_dir_all(&bin).unwrap();
        let script = "#!/bin/sh\nif [ \"$1\" = '--version' ]; then echo wine-test; exit 0; fi\nif [ \"$1\" = wineboot ]; then mkdir -p \"$WINEPREFIX/drive_c\"; touch \"$WINEPREFIX/system.reg\"; exit 0; fi\nprintf '%s\\n' \"$WINEPREFIX\" \"$@\"\n";
        for name in ["wine", "wineserver"] { let p=bin.join(name); fs::write(&p,script).unwrap(); fs::set_permissions(p, fs::Permissions::from_mode(0o755)).unwrap(); }
        let e=dispatch(&s,req(json!({"op":"add_engine","path":bin.join("wine")}))).unwrap();
        let b=dispatch(&s,req(json!({"op":"create","name":"Test","engine_id":e["id"],"dpi":192}))).unwrap();
        let id=b["id"].as_str().unwrap(); assert_eq!(s.pair(id).unwrap().0.status,"ready");
        assert_eq!(s.pair(id).unwrap().0.dpi, Some(192));
        let scale=dispatch(&s,req(json!({"op":"read_log","id":id}))).unwrap();
        assert!(scale["text"].as_str().unwrap().contains("HKCU\\Control Panel\\Desktop\n/v\nLogPixels\n/t\nREG_DWORD\n/d\n192"));
        dispatch(&s,req(json!({"op":"set_dpi","id":id,"dpi":144}))).unwrap();
        assert_eq!(s.pair(id).unwrap().0.dpi, Some(144));
        assert!(dispatch(&s,req(json!({"op":"set_dpi","id":id,"dpi":0}))).is_err());
        assert_eq!(s.pair(id).unwrap().0.dpi, Some(144));
        let exe=s.root.join("app with spaces.exe"); fs::write(&exe,[]).unwrap();
        dispatch(&s,req(json!({"op":"launch","id":id,"path":exe,"arguments":["literal ; $HOME"]}))).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        let log=dispatch(&s,req(json!({"op":"read_log","id":id}))).unwrap();
        assert!(log["text"].as_str().unwrap().contains("literal ; $HOME"));
        assert!(log["text"].as_str().unwrap().contains(s.prefix(id).unwrap().to_str().unwrap()));
        dispatch(&s,req(json!({"op":"stop","id":id}))).unwrap();
        fs::write(bin.join("wineserver"), "#!/bin/sh\nexit 1\n").unwrap();
        let idle = dispatch(&s,req(json!({"op":"stop","id":id}))).unwrap();
        assert!(idle["message"].as_str().unwrap().contains("уже остановлено"));
        fs::remove_dir_all(s.root).unwrap();
    }
}
