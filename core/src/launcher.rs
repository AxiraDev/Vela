use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub website: String,
    pub download: Option<String>,
    pub filename: String,
    pub download_hosts: Vec<String>,
    pub requires_wow64: bool,
    pub note: String,
    pub executables: Vec<String>,
    #[serde(default)]
    pub arguments: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Installation {
    pub launcher_id: String,
    pub status: String,
    pub installer_path: String,
    pub client_path: Option<String>,
}

pub(crate) fn catalog() -> Result<Vec<Entry>> {
    serde_json::from_str(include_str!("../resources/launchers.json")).map_err(|e| local(format!("Ошибка каталога: {e}"), format!("Catalog error: {e}")))
}

fn validate_installer(path: &Path, entry: &Entry, engine: &Engine) -> Result<()> {
    if entry.requires_wow64 && engine.supports_x86_32 == Some(false) {
        return Err(local("Установщик этого лаунчера требует поддержки Windows 32/64 бит (WoW64). Выбранный движок поддерживает только 64 бит. Подключите совместимый движок в разделе «Движки Wine».", "This launcher's installer requires Windows 32/64-bit support (WoW64). The selected engine supports 64-bit programs only. Add a compatible engine under Wine engines.").into());
    }
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size < 64 || size > 512 * 1024 * 1024 { return Err(local("Недопустимый размер установщика (максимум 512 МБ)", "Invalid installer size (maximum 512 MiB)").into()); }
    let expected = Path::new(&entry.filename).extension().and_then(|s| s.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case(expected) { return Err(local(format!("Для этого лаунчера нужен установщик {expected}"), format!("This launcher needs an {expected} installer"))); }
    match expected {
        "exe" => {
            let machine = engine::pe_machine(path).ok_or(local("Сервер вернул не установщик Windows. Загрузите файл с официального сайта.", "The server did not return a Windows installer. Download the file from the official website."))?;
            if ![0x014c, 0x8664].contains(&machine) { return Err(local("Нужен установщик Windows x86 или x64", "Choose a Windows x86 or x64 installer").into()); }
            if machine == 0x014c && engine.supports_x86_32 == Some(false) { return Err(local("Этот установщик 32-битный. Выберите движок с поддержкой WoW64.", "This installer is 32-bit. Choose an engine with WoW64 support.").into()); }
        }
        "msi" => {
            let mut magic = [0; 8]; File::open(path).and_then(|mut f| f.read_exact(&mut magic)).map_err(|e| e.to_string())?;
            if magic != [0xd0,0xcf,0x11,0xe0,0xa1,0xb1,0x1a,0xe1] { return Err(local("Загруженный файл не является MSI-контейнером", "The downloaded file is not an MSI container").into()); }
        }
        _ => return Err(local("Для этого лаунчера нет автоматической установки", "This launcher does not have an automatic installer preset").into()),
    }
    Ok(())
}

pub(crate) fn install(store: &Store, r: &Request) -> Result<Value> {
    let entry = catalog()?.into_iter().find(|e| e.id == r.launcher_id).ok_or(local("Лаунчер не найден", "Launcher not found"))?;
    if entry.download.is_none() { return Err(local("Для этого лаунчера автоматическая установка недоступна", "Automatic installation is unavailable for this launcher").into()); }
    let engine = store.read()?.engines.into_iter().find(|e| e.id == r.engine_id).ok_or(local("Выберите движок Wine", "Choose a Wine engine"))?;
    let name = clean_name(&r.name)?;
    if let Some(dpi) = r.dpi { validate_dpi(dpi)?; }
    let source = absolute_file(&r.path)?;
    validate_installer(&source, &entry, &engine)?;
    // Keep the installer in user data, never in the signed application bundle.
    let id = uid();
    let cache = store.root.join("installers").join(&id);
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let installer = cache.join(&entry.filename);
    if let Err(e) = fs::copy(source, &installer) { let _ = fs::remove_dir_all(&cache); return Err(e.to_string()); }
    if let Err(e) = validate_installer(&installer, &entry, &engine) { let _ = fs::remove_dir_all(&cache); return Err(e); }
    let b = Bottle { id, name, engine_id: engine.id.clone(), status: "initializing".into(),
        environment: BTreeMap::new(), shortcuts: vec![], last_log: None, dpi: r.dpi,
        installation: Some(Installation { launcher_id: entry.id, status: "pending".into(), installer_path: installer.to_string_lossy().into_owned(), client_path: None }) };
    if let Err(e) = store.update(|s| {
        fs::create_dir(store.prefix(&b.id)?).map_err(|e| e.to_string())?;
        s.bottles.push(b.clone()); Ok(())
    }) { let _ = fs::remove_dir_all(cache); return Err(e); }
    // Retain a failed environment and cached installer so the user can retry.
    if let Err(e) = initialize(store, &b.id) {
        store.update(|s| { bottle_mut(s, &b.id)?.installation.as_mut().unwrap().status = "failed".into(); Ok(()) })?;
        return Err(e);
    }
    start(store, &b.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    fn pe(path: &Path, machine: u16) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut b = vec![0; 134]; b[..2].copy_from_slice(b"MZ"); b[60..64].copy_from_slice(&128u32.to_le_bytes());
        b[128..132].copy_from_slice(b"PE\0\0"); b[132..134].copy_from_slice(&machine.to_le_bytes()); fs::write(path,b).unwrap();
    }
    fn setup() -> (Store, String) {
        let store = Store::new(std::env::temp_dir().join(format!("vela-launcher-{}",uid()))).unwrap();
        let bin = store.root.join("engine/bin"); fs::create_dir_all(&bin).unwrap();
        let wine = bin.join("wine");
        fs::write(&wine, "#!/bin/sh\nif [ \"$1\" = '--version' ]; then echo wine-fixture; exit; fi\nif [ \"$1\" = wineboot ]; then mkdir -p \"$WINEPREFIX/drive_c\"; touch \"$WINEPREFIX/system.reg\"; exit; fi\nprintf '%s\\n' \"$WINEPREFIX\" \"$@\"\n").unwrap();
        fs::set_permissions(&wine, fs::Permissions::from_mode(0o755)).unwrap(); fs::copy(&wine,bin.join("wineserver")).unwrap();
        let value = dispatch(&store, serde_json::from_value(json!({"op":"add_engine","path":wine})).unwrap()).unwrap();
        (store, value["id"].as_str().unwrap().into())
    }
    #[test]
    fn official_catalog_is_unique_and_has_bounded_relative_detection_paths() {
        let entries = catalog().unwrap(); let mut ids = std::collections::BTreeSet::new();
        for entry in entries {
            assert!(ids.insert(entry.id)); assert!(entry.website.starts_with("https://"));
            if let Some(download) = entry.download {
                assert!(download.starts_with("https://")); assert!(!entry.download_hosts.is_empty()); assert!(!entry.executables.is_empty());
            }
            for relative in entry.executables {
                assert!(Path::new(&relative).components().all(|c| matches!(c,std::path::Component::Normal(_))));
            }
        }
    }
    #[test]
    fn install_creates_one_environment_and_detects_client_idempotently() {
        let (s,engine) = setup(); let installer = s.root.join("SteamSetup.exe"); pe(&installer,0x014c);
        let r: Request = serde_json::from_value(json!({"op":"install_launcher","launcher_id":"steam","engine_id":engine,"name":"Steam test","path":installer,"dpi":192})).unwrap();
        let result = install(&s,&r).unwrap(); let id = result["id"].as_str().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let (b,_) = s.pair(id).unwrap(); assert_eq!(b.status,"ready"); assert_eq!(b.installation.as_ref().unwrap().status,"pending");
        let log = fs::read_to_string(s.root.join("logs").join(b.last_log.unwrap())).unwrap();
        assert!(log.contains(s.prefix(id).unwrap().to_str().unwrap())); assert!(log.contains("installers/"));
        fs::remove_file(installer).unwrap(); retry(&s,id).unwrap();
        assert_eq!(s.read().unwrap().bottles.len(),1);
        let client = s.prefix(id).unwrap().join("drive_c/Program Files (x86)/Steam/steam.exe"); pe(&client,0x014c);
        reconcile(&s).unwrap(); reconcile(&s).unwrap();
        let (b,_) = s.pair(id).unwrap(); assert_eq!(b.shortcuts.len(),1); assert_eq!(b.shortcuts[0].executable,client.to_str().unwrap());
        assert_eq!(b.installation.unwrap().status,"detected");
        fs::remove_dir_all(s.root).unwrap();
    }
    #[test]
    fn detected_itch_shortcut_uses_stable_update_entry_point() {
        let (s,engine) = setup(); let installer=s.root.join("setup.exe"); pe(&installer,0x8664);
        let request: Request=serde_json::from_value(json!({"op":"install_launcher","launcher_id":"itch","engine_id":engine,"name":"itch test","path":installer})).unwrap();
        let installed=install(&s,&request).unwrap(); let id=installed["id"].as_str().unwrap();
        let client=s.prefix(id).unwrap().join("drive_c/users/player/AppData/Local/itch/itch-setup.exe"); pe(&client,0x8664);
        reconcile(&s).unwrap();
        let (b,_)=s.pair(id).unwrap(); assert_eq!(b.shortcuts.len(),1);
        assert_eq!(b.shortcuts[0].arguments,vec!["--prefer-launch","--appname","itch"]);
        assert_eq!(b.shortcuts[0].executable,client.to_str().unwrap());
        fs::remove_dir_all(s.root).unwrap();
    }
    #[test]
    fn invalid_download_and_incompatible_engine_do_not_create_environments() {
        let (s,id) = setup(); let path = s.root.join("setup.exe"); fs::write(&path, "<html>not an installer</html>".repeat(8)).unwrap();
        let mut r: Request = serde_json::from_value(json!({"op":"install_launcher","launcher_id":"steam","engine_id":id,"name":"bad","path":path})).unwrap();
        assert!(install(&s,&r).is_err()); assert!(s.read().unwrap().bottles.is_empty());
        pe(&path,0x014c);
        fs::create_dir_all(s.root.join("engine/lib/wine")).unwrap();
        fs::write(s.root.join("engine/vela-runtime.json"), r#"{"supports_x86_32":false}"#).unwrap();
        assert!(install(&s,&r).is_err()); assert!(s.read().unwrap().bottles.is_empty());
        r.launcher_id="riot".into(); assert!(install(&s,&r).is_err());
        fs::remove_dir_all(s.root).unwrap();
    }
    #[test]
    fn detection_never_follows_symlink_outside_environment() {
        let root = std::env::temp_dir().join(format!("vela-detect-{}",uid()));
        let prefix = root.join("prefix"); let drive=prefix.join("drive_c"); let outside=root.join("outside");
        fs::create_dir_all(drive.join("Program Files (x86)")).unwrap(); pe(&outside.join("steam.exe"),0x014c);
        symlink(&outside,drive.join("Program Files (x86)/Steam")).unwrap();
        let entry=catalog().unwrap().into_iter().find(|e| e.id=="steam").unwrap();
        assert!(detect(&prefix,&entry).is_none());
        fs::remove_dir_all(root).unwrap();
    }
}

fn start(store: &Store, id: &str) -> Result<Value> {
    let (b, e) = store.pair(id)?;
    let installation = b.installation.as_ref().ok_or(local("В этом окружении нет установки лаунчера", "This environment has no launcher installation"))?;
    let path = absolute_file(&installation.installer_path)?;
    let entry = catalog()?.into_iter().find(|e| e.id == installation.launcher_id).ok_or(local("Лаунчер не найден", "Launcher not found"))?;
    validate_installer(&path, &entry, &e)?;
    let result = launch(store, &b, &e, &executable_args(&path, &[])?, path.parent().and_then(Path::to_str), &local(format!("Установка {}", entry.name), format!("Install {}", entry.name)));
    store.update(|s| {
        let i = bottle_mut(s, id)?.installation.as_mut().ok_or(local("Установка не найдена", "Installation not found"))?;
        i.status = if result.is_ok() { "pending" } else { "failed" }.into(); Ok(())
    })?;
    let mut value = result?;
    value["id"] = json!(id);
    Ok(value)
}

pub(crate) fn retry(store: &Store, id: &str) -> Result<Value> {
    if store.pair(id)?.0.status != "ready" { initialize(store, id)?; }
    start(store, id)
}

// Only a small set of known relative paths is inspected. Never traverse Z: or
// follow a link outside drive_c, even if an installer creates one.
fn detect(prefix: &Path, entry: &Entry) -> Option<PathBuf> {
    let drive = fs::canonicalize(prefix.join("drive_c")).ok()?;
    if !drive.starts_with(fs::canonicalize(prefix).ok()?) { return None; }
    let users: Vec<String> = fs::read_dir(drive.join("users")).ok().into_iter().flatten().filter_map(|e| e.ok())
        .take(64).filter_map(|e| e.file_name().into_string().ok()).collect();
    for template in &entry.executables {
        let paths: Vec<String> = if template.contains("{user}") { users.iter().map(|u| template.replace("{user}", u)).collect() } else { vec![template.clone()] };
        for relative in paths {
            if Path::new(&relative).components().any(|c| !matches!(c, std::path::Component::Normal(_))) { continue; }
            let Ok(candidate) = fs::canonicalize(drive.join(relative)) else { continue; };
            if candidate.starts_with(&drive) && candidate.is_file() && engine::pe_machine(&candidate).is_some() { return Some(candidate); }
        }
    }
    None
}

pub(crate) fn reconcile(store: &Store) -> Result<()> {
    let state = store.read()?;
    let entries = catalog()?;
    let mut found = Vec::new();
    for bottle in &state.bottles {
        let Some(i) = &bottle.installation else { continue; };
        if i.status == "detected" || bottle.status != "ready" { continue; }
        let Some(entry) = entries.iter().find(|e| e.id == i.launcher_id) else { continue; };
        if let Some(path) = detect(&store.prefix(&bottle.id)?, entry) { found.push((bottle.id.clone(), entry.name.clone(), path, entry.arguments.clone())); }
    }
    if found.is_empty() { return Ok(()); }
    store.update(|s| {
        for (id, name, path, arguments) in &found {
            let b = bottle_mut(s, id)?;
            let executable = path.to_string_lossy().into_owned();
            if !b.shortcuts.iter().any(|s| s.executable == executable) {
                b.shortcuts.push(Shortcut { id: uid(), name: name.clone(), executable: executable.clone(), arguments: arguments.clone(), directory: path.parent().map(|p| p.to_string_lossy().into_owned()) });
            }
            if let Some(i) = b.installation.as_mut() { i.status = "detected".into(); i.client_path = Some(executable); }
        }
        Ok(())
    })
}
