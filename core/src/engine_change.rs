use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Backup {
    pub id: String,
    pub engine_name: String,
}

pub(crate) fn ensure_idle(b: &Bottle) -> Result<()> {
    if b.pending_engine.is_some() {
        return Err(local("Сначала восстановите окружение после прерванной смены движка.", "Restore the environment after the interrupted engine change first.").into());
    }
    Ok(())
}

pub(crate) fn backup_root(store: &Store, id: &str) -> Result<PathBuf> {
    store.prefix(id)?;
    let parent = store.root.join("engine-backups");
    let root = parent.join(id);
    for p in [&parent, &root] {
        if p.is_symlink() || (p.exists() && !p.is_dir()) {
            return Err(local("Некорректная папка резервных копий", "Invalid engine backup folder").into());
        }
    }
    Ok(root)
}

fn backup_path(store: &Store, id: &str, backup: &Backup) -> Result<PathBuf> {
    // Backup IDs use the same bounded path component format as environment IDs.
    store.prefix(&backup.id)?;
    let path = backup_root(store, id)?.join(&backup.id);
    if path.is_symlink() { return Err(local("Резервная копия не должна быть символической ссылкой", "The backup must not be a symbolic link").into()); }
    Ok(path)
}

// ditto preserves Wine's symlinks as links, along with file metadata. APFS clones
// share unchanged blocks where supported; other filesystems use ordinary copies.
fn copy_prefix(source: &Path, destination: &Path, log: &File) -> Result<()> {
    if source.is_symlink() || !source.is_dir() || destination.exists() || destination.is_symlink() {
        return Err(local("Некорректный путь копии окружения", "Invalid environment copy path").into());
    }
    let mut child = Command::new("/usr/bin/ditto").env_clear()
        .env("PATH", "/usr/bin:/bin").env("DITTOABORT", "1")
        .arg("--clone").arg(source).arg(destination).stdin(Stdio::null())
        .stdout(Stdio::null()).stderr(log.try_clone().map_err(|e| e.to_string())?)
        .spawn().map_err(|e| e.to_string())?;
    if !wait_bounded(&mut child, Duration::from_secs(900))?.success() {
        return Err(local("Не удалось скопировать окружение. Проверьте свободное место и журнал.", "Could not copy the environment. Check free disk space and the log.").into());
    }
    Ok(())
}

pub(crate) fn stop_all(store: &Store, b: &Bottle) -> Result<()> {
    if !store.prefix(&b.id)?.exists() { return Ok(()); }
    let state = store.read()?;
    let mut ids = Vec::new();
    if let Some(pending) = b.pending_engine.as_deref() {
        if pending != b.engine_id { ids.push(pending); }
    }
    ids.push(b.engine_id.as_str());
    let mut failed = Vec::new();
    for id in ids {
        let e = state.engines.iter().find(|e| e.id == id).ok_or(local("Движок не найден", "Engine not found"))?;
        if stop_environment(store, b, e).is_err() { failed.push(e); }
    }
    // A different Wine version may initially report a server mismatch. Give the
    // owning engine a chance to stop it before retrying the other version.
    for e in failed { stop_environment(store, b, e)?; }
    for id in [Some(b.engine_id.as_str()), b.pending_engine.as_deref()].into_iter().flatten() {
        let e = state.engines.iter().find(|e| e.id == id).ok_or(local("Движок не найден", "Engine not found"))?;
        removal::wait_for_server(store, b, e)?;
    }
    Ok(())
}

fn update_prefix(store: &Store, b: &Bottle, e: &Engine, log: &File) -> Result<()> {
    let mut c = wine_command(store, b, e)?;
    let overrides = b.environment.get("WINEDLLOVERRIDES").map(String::as_str).unwrap_or("winemenubuilder.exe=d");
    c.env("WINEDLLOVERRIDES", format!("{overrides};mscoree,mshtml=d"));
    c.args(["wineboot", "--update"]).stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log.try_clone().map_err(|e| e.to_string())?);
    if !wait_bounded(&mut c.spawn().map_err(|e| e.to_string())?, Duration::from_secs(120))?.success() {
        return Err(local("Новый движок не смог обновить окружение", "The new engine could not update the environment").into());
    }
    if !store.prefix(&b.id)?.join("system.reg").is_file() {
        return Err(local("Не найден реестр окружения", "The environment registry was not found").into());
    }
    if let Some(dpi) = b.dpi { apply_dpi(store, b, e, dpi)?; }
    let mut c = wine_command(store, b, e)?;
    c.args(["cmd.exe", "/d", "/c", "exit", "0"])
        .stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log.try_clone().map_err(|e| e.to_string())?);
    if !wait_bounded(&mut c.spawn().map_err(|e| e.to_string())?, Duration::from_secs(20))?.success() {
        return Err(local("Новый движок не прошёл проверку запуска Windows", "The new engine failed the Windows execution check").into());
    }
    stop_environment(store, b, e)?;
    removal::wait_for_server(store, b, e)
}

pub(crate) fn change(store: &Store, id: &str, engine_id: &str) -> Result<Value> {
    let _operation = store.operation_lock(id)?;
    let (b, old) = store.pair(id)?;
    ensure_idle(&b)?;
    if b.engine_id == engine_id { return Ok(json!({"id":id})); }
    if b.status != "ready" { return Err(local("Сначала завершите настройку или восстановление окружения", "Finish setting up or restoring the environment first").into()); }
    let state = store.read()?;
    let target = state.engines.iter().find(|e| e.id == engine_id).ok_or(local("Движок не найден", "Engine not found"))?;
    probe(store, target)?;
    let prefix = store.prefix(id)?;
    if !prefix.join("system.reg").is_file() { return Err(local("Файлы окружения не найдены", "The environment files are missing").into()); }
    let mut header = Vec::new();
    File::open(prefix.join("system.reg")).and_then(|f| f.take(4096).read_to_end(&mut header)).map_err(|e| e.to_string())?;
    if String::from_utf8_lossy(&header).lines().any(|line| line.trim() == "#arch=win32") {
        return Err(local("Смена движка не преобразует 32-битное окружение в 64-битное.", "Changing the engine cannot convert a 32-bit prefix into a 64-bit prefix.").into());
    }
    let backup = Backup { id: uid(), engine_name: old.name };
    let path = backup_path(store, id, &backup)?;
    stop_all(store, &b)?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    let (log_path, mut log) = store.log(&b, "Change Wine engine")?;
    writeln!(log, "From: {}\nTo: {}\nBackup: {}", b.engine_id, engine_id, path.display()).map_err(|e| e.to_string())?;
    copy_prefix(&prefix, &path.join("prefix"), &log)?;
    let mut metadata = File::create(path.join("bottle.json")).map_err(|e| e.to_string())?;
    metadata.write_all(&serde_json::to_vec_pretty(&b).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    metadata.sync_all().map_err(|e| e.to_string())?;
    // Persist recovery information BEFORE Wine can alter any prefix files.
    store.update(|s| {
        let b = bottle_mut(s, id)?;
        b.engine_backup = Some(backup.clone()); b.pending_engine = Some(engine_id.into()); b.status = "changing_engine".into(); Ok(())
    })?;
    match update_prefix(store, &b, target, &log) {
        Ok(()) => {
            store.update(|s| { let b = bottle_mut(s, id)?; b.engine_id = engine_id.into(); b.pending_engine = None; b.status = "ready".into(); b.last_log = log_path.file_name().map(|n| n.to_string_lossy().into_owned()); Ok(()) })?;
            Ok(json!({"id":id,"backup":path,"log":log_path}))
        }
        Err(error) => {
            writeln!(log, "Engine change failed: {error}").ok();
            match restore_locked(store, id) {
                Ok(_) => {
                    let _ = store.update(|s| { bottle_mut(s, id)?.last_log = log_path.file_name().map(|n| n.to_string_lossy().into_owned()); Ok(()) });
                    Err(local(format!("{error}. Предыдущее окружение восстановлено из резервной копии. Журнал: {}", log_path.display()), format!("{error}. The previous environment was restored from its backup. Log: {}", log_path.display())))
                }
                Err(recovery) => Err(local(format!("{error}. Автовосстановление не завершено: {recovery}. В настройках окружения доступно восстановление. Копия: {}", path.display()), format!("{error}. Automatic recovery did not finish: {recovery}. Use Restore in environment settings. Backup: {}", path.display()))),
            }
        }
    }
}

pub(crate) fn restore(store: &Store, id: &str) -> Result<Value> {
    let _operation = store.operation_lock(id)?;
    restore_locked(store, id)
}

fn restore_locked(store: &Store, id: &str) -> Result<Value> {
    let state = store.read()?;
    let current = state.bottles.iter().find(|b| b.id == id).ok_or(local("Окружение не найдено", "Environment not found"))?;
    let backup = current.engine_backup.as_ref().ok_or(local("Резервная копия не найдена", "No engine backup was found"))?;
    let path = backup_path(store, id, backup)?;
    if path.join("bottle.json").is_symlink() { return Err(local("Некорректные данные резервной копии", "Invalid backup metadata").into()); }
    let mut original: Bottle = serde_json::from_slice(&fs::read(path.join("bottle.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if original.id != id || original.pending_engine.is_some() { return Err(local("Резервная копия принадлежит другому окружению или не завершена", "The backup belongs to another environment or is incomplete").into()); }
    let engine = state.engines.iter().find(|e| e.id == original.engine_id).ok_or(local("Предыдущий движок не найден", "The previous engine was not found"))?;
    probe(store, engine)?;
    stop_all(store, current)?;
    let prefix = store.prefix(id)?;
    let (_, log) = store.log(current, "Restore environment before engine change")?;
    let staged = path.join(format!("restore-{}", uid()));
    copy_prefix(&path.join("prefix"), &staged, &log)?;
    store.update(|s| { let b = bottle_mut(s, id)?; b.pending_engine = Some(current.engine_id.clone()); b.status = "restoring_engine".into(); Ok(()) })?;
    // Retain the replaced prefix too. Recovery never deletes post-switch files.
    let replaced = path.join(format!("replaced-{}", uid()));
    if prefix.exists() { fs::rename(&prefix, &replaced).map_err(|e| e.to_string())?; }
    if let Err(error) = fs::rename(&staged, &prefix) {
        if replaced.exists() { let _ = fs::rename(&replaced, &prefix); }
        return Err(error.to_string());
    }
    original.engine_backup = Some(backup.clone());
    original.pending_engine = None;
    original.status = "ready".into();
    store.update(|s| { *bottle_mut(s, id)? = original; Ok(()) })?;
    Ok(json!({"id":id,"backup":path}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

    fn setup(fail_update: bool) -> (Store, String, String, String) {
        let store = Store::new(std::env::temp_dir().join(format!("vela-engine-change-{}", uid()))).unwrap();
        let mut engines = Vec::new();
        for (name, fail) in [("old", false), ("new", fail_update)] {
            let bin = store.root.join(name).join("bin"); fs::create_dir_all(&bin).unwrap();
            let script = format!("#!/bin/sh\nif [ \"$1\" = --version ]; then echo wine-{name}; exit; fi\nif [ \"$1\" = wineboot ]; then mkdir -p \"$WINEPREFIX/drive_c\"; touch \"$WINEPREFIX/system.reg\"; echo {name} > \"$WINEPREFIX/drive_c/engine.txt\"; if [ \"$2\" = --update ]; then exit {}; fi; exit; fi\nprintf '%s\\n' {name} \"$WINEPREFIX\" \"$@\"\n", if fail { 23 } else { 0 });
            for file in ["wine", "wineserver"] {
                let path = bin.join(file);
                fs::write(&path, &script).unwrap(); fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            }
            let e = dispatch(&store, serde_json::from_value(json!({"op":"add_engine","path":bin.join("wine")})).unwrap()).unwrap();
            engines.push(e["id"].as_str().unwrap().to_string());
        }
        let b = dispatch(&store, serde_json::from_value(json!({"op":"create","name":"Saved apps","engine_id":engines[0],"dpi":192})).unwrap()).unwrap();
        (store, b["id"].as_str().unwrap().into(), engines[0].clone(), engines[1].clone())
    }

    #[test]
    fn changing_and_restoring_preserves_programs_settings_and_host_links() {
        let (s, id, old, new) = setup(false);
        let prefix = s.prefix(&id).unwrap();
        let data = prefix.join("drive_c/save.dat"); fs::write(&data, "before").unwrap();
        let host = s.root.join("host"); fs::create_dir(&host).unwrap(); fs::write(host.join("keep"), "host").unwrap();
        symlink(&host, prefix.join("drive_c/host-link")).unwrap();
        s.update(|state| {
            let b = bottle_mut(state, &id)?;
            b.environment.insert("MY_SETTING".into(), "keep".into());
            b.shortcuts.push(Shortcut { id:uid(), name:"App".into(), executable:data.to_string_lossy().into_owned(), arguments:vec!["literal".into()], directory:None }); Ok(())
        }).unwrap();
        change(&s, &id, &new).unwrap();
        let (b, _) = s.pair(&id).unwrap(); assert_eq!(b.engine_id, new); assert_eq!(b.status, "ready"); assert!(b.pending_engine.is_none());
        assert_eq!(b.environment["MY_SETTING"], "keep"); assert_eq!(b.dpi, Some(192)); assert_eq!(b.shortcuts.len(), 1);
        assert_eq!(fs::read_to_string(&data).unwrap(), "before");
        assert_eq!(fs::read_to_string(prefix.join("drive_c/engine.txt")).unwrap(), "new\n");
        let backup = backup_path(&s, &id, b.engine_backup.as_ref().unwrap()).unwrap();
        assert!(backup.join("prefix/drive_c/host-link").is_symlink());
        fs::write(&data, "after").unwrap(); restore(&s, &id).unwrap();
        assert_eq!(s.pair(&id).unwrap().0.engine_id, old);
        assert_eq!(fs::read_to_string(&data).unwrap(), "before");
        assert_eq!(fs::read_to_string(prefix.join("drive_c/engine.txt")).unwrap(), "old\n");
        let retained = fs::read_dir(backup).unwrap().filter_map(|e| e.ok()).find(|e| e.file_name().to_string_lossy().starts_with("replaced-")).unwrap();
        assert_eq!(fs::read_to_string(retained.path().join("drive_c/save.dat")).unwrap(), "after");
        assert_eq!(fs::read_to_string(host.join("keep")).unwrap(), "host");
        removal::trash(&s, &id).unwrap(); removal::purge(&s, &id).unwrap();
        assert!(!backup_root(&s, &id).unwrap().exists());
        assert_eq!(fs::read_to_string(host.join("keep")).unwrap(), "host");
        fs::remove_dir_all(s.root).unwrap();
    }

    #[test]
    fn failed_update_rolls_back_and_busy_or_invalid_requests_do_not_change_files() {
        let (s, id, old, new) = setup(true);
        let prefix = s.prefix(&id).unwrap();
        fs::write(prefix.join("drive_c/save.dat"), "keep").unwrap();
        { let _busy = s.operation_lock(&id).unwrap(); assert!(change(&s, &id, &new).is_err()); }
        assert!(change(&s, &id, "missing").is_err());
        assert!(!backup_root(&s, &id).unwrap().exists());
        let error = change(&s, &id, &new).unwrap_err(); assert!(error.contains("восстановлено") || error.contains("restored"));
        let (b, _) = s.pair(&id).unwrap(); assert_eq!(b.engine_id, old); assert_eq!(b.status, "ready"); assert!(b.pending_engine.is_none());
        assert_eq!(fs::read_to_string(prefix.join("drive_c/engine.txt")).unwrap(), "old\n");
        assert_eq!(fs::read_to_string(prefix.join("drive_c/save.dat")).unwrap(), "keep");
        fs::remove_dir_all(s.root).unwrap();
    }

    #[test]
    fn interrupted_change_is_blocked_and_recoverable_even_between_directory_renames() {
        let (s, id, old, new) = setup(false);
        change(&s, &id, &new).unwrap();
        let (b, _) = s.pair(&id).unwrap();
        let backup = backup_path(&s, &id, b.engine_backup.as_ref().unwrap()).unwrap();
        s.update(|state| { let b = bottle_mut(state, &id)?; b.pending_engine = Some(new.clone()); b.status = "changing_engine".into(); Ok(()) }).unwrap();
        assert!(initialize(&s, &id).is_err());
        assert!(launch(&s, &b, &["cmd.exe".into()], None, "blocked").is_err());
        fs::rename(s.prefix(&id).unwrap(), backup.join("replaced-crash")).unwrap();
        restore(&s, &id).unwrap(); assert_eq!(s.pair(&id).unwrap().0.engine_id, old);
        assert!(s.prefix(&id).unwrap().join("system.reg").exists());
        fs::remove_dir_all(s.root).unwrap();
    }

    #[test]
    fn backup_path_escape_and_failed_stop_preserve_original_environment() {
        let (s, id, old, new) = setup(false);
        symlink(s.root.join("old"), s.root.join("engine-backups")).unwrap();
        assert!(change(&s, &id, &new).is_err());
        fs::remove_file(s.root.join("engine-backups")).unwrap();
        fs::write(s.root.join("old/bin/wineserver"), "#!/bin/sh\nexit 2\n").unwrap();
        assert!(change(&s, &id, &new).is_err());
        assert_eq!(s.pair(&id).unwrap().0.engine_id, old);
        assert!(!s.root.join("engine-backups").exists());
        fs::remove_dir_all(s.root).unwrap();
    }

    #[test]
    fn launch_rechecks_the_selected_engine_after_acquiring_the_operation_lock() {
        let (s, id, _, new) = setup(false);
        let (stale_bottle, _) = s.pair(&id).unwrap();
        fs::create_dir_all(s.root.join("new/lib/wine")).unwrap();
        fs::write(s.root.join("new/vela-runtime.json"), r#"{"supports_x86_32":false}"#).unwrap();
        change(&s, &id, &new).unwrap();
        let exe = s.root.join("x86.exe");
        let mut pe = vec![0;134]; pe[..2].copy_from_slice(b"MZ"); pe[60..64].copy_from_slice(&128u32.to_le_bytes());
        pe[128..132].copy_from_slice(b"PE\0\0"); pe[132..134].copy_from_slice(&0x014cu16.to_le_bytes()); fs::write(&exe,pe).unwrap();
        assert!(launch(&s, &stale_bottle, &[exe.to_string_lossy().into_owned()], None, "stale launch").is_err());
        fs::remove_dir_all(s.root).unwrap();
    }
}
