use super::*;

pub(crate) fn trash(store: &Store, id: &str) -> Result<Value> {
    let _operation = store.operation_lock(id)?;
    let (b, _) = store.pair(id)?;
    // Wait for the prefix's Wine server to exit before hiding it from the library.
    engine_change::stop_all(store, &b)?;
    store.update(|s| {
        let index = s.bottles.iter().position(|b| b.id == id).ok_or(local("Окружение не найдено", "Environment not found"))?;
        let b = s.bottles.remove(index);
        s.deleted_bottles.push(b);
        Ok(())
    })?;
    Ok(json!({"id":id}))
}

pub(crate) fn wait_for_server(store: &Store, b: &Bottle, e: &Engine) -> Result<()> {
    let server = if e.server.is_empty() { Path::new(&e.wine).parent().ok_or(local("Не найдена папка Wine", "Wine folder not found"))?.join("wineserver") } else { PathBuf::from(&e.server) };
    let mut command = runtime_command(e, &server.to_string_lossy())?;
    command.env("WINEPREFIX", store.prefix(&b.id)?).arg("-w").stdout(Stdio::null()).stderr(Stdio::null());
    let status = wait_bounded(&mut command.spawn().map_err(|e| e.to_string())?, Duration::from_secs(15))?;
    if !status.success() { return Err(local("Не удалось дождаться остановки Wine. Окружение сохранено.", "Wine did not stop successfully. The environment has been kept.").into()); }
    Ok(())
}

pub(crate) fn restore(store: &Store, id: &str) -> Result<Value> {
    let _operation = store.operation_lock(id)?;
    if !store.prefix(id)?.is_dir() { return Err(local("Файлы окружения не найдены. Восстановление невозможно.", "The environment files are missing and cannot be restored.").into()); }
    store.update(|s| {
        if s.bottles.iter().any(|b| b.id == id) { return Err(local("Окружение уже восстановлено", "The environment has already been restored").into()); }
        let index = s.deleted_bottles.iter().position(|b| b.id == id).ok_or(local("Окружение не найдено в корзине", "The environment was not found in Vela Trash"))?;
        let b = s.deleted_bottles.remove(index); s.bottles.push(b); Ok(())
    })?;
    Ok(json!({"id":id}))
}

pub(crate) fn purge(store: &Store, id: &str) -> Result<Value> {
    let _operation = store.operation_lock(id)?;
    let state = store.read()?;
    let b = state.deleted_bottles.iter().find(|b| b.id == id).ok_or(local("Сначала переместите окружение в корзину", "Move the environment to Vela Trash first"))?;
    let prefix = store.prefix(id)?;
    let backups = engine_change::backup_root(store, id)?;
    let installers_root = store.root.join("installers");
    if store.root.join("logs").is_symlink() {
        return Err(local("Папка журналов не должна быть символической ссылкой", "The log folder must not be a symbolic link").into());
    }
    if installers_root.is_symlink() || installers_root.join(id).is_symlink() {
        return Err(local("Папка установщиков не должна быть символической ссылкой", "The installer folder must not be a symbolic link").into());
    }
    if prefix.exists() {
        engine_change::stop_all(store, b)?;
        // remove_dir_all does not follow Wine's symlinks into the host filesystem.
        fs::remove_dir_all(prefix).map_err(|e| e.to_string())?;
    }
    let installers = installers_root.join(id);
    if installers.exists() { fs::remove_dir_all(installers).map_err(|e| e.to_string())?; }
    if backups.exists() { fs::remove_dir_all(backups).map_err(|e| e.to_string())?; }
    for entry in fs::read_dir(store.root.join("logs")).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&format!("{id}-")) && name.ends_with(".log") && !entry.path().is_dir() {
            fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    store.update(|s| { s.deleted_bottles.retain(|b| b.id != id); Ok(()) })?;
    Ok(json!({"id":id}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    fn setup() -> (Store, String, String) {
        let s = Store::new(std::env::temp_dir().join(format!("vela-removal-{}",uid()))).unwrap();
        let bin=s.root.join("engine/bin");fs::create_dir_all(&bin).unwrap();
        let script="#!/bin/sh\nif [ \"$1\" = '--version' ]; then echo wine-fixture; exit; fi\nif [ \"$1\" = wineboot ]; then mkdir -p \"$WINEPREFIX/drive_c\"; touch \"$WINEPREFIX/system.reg\"; exit; fi\nprintf '%s\\n' \"$WINEPREFIX\" \"$@\"\n";
        for name in ["wine","wineserver"] { let path=bin.join(name);fs::write(&path,script).unwrap();fs::set_permissions(path,fs::Permissions::from_mode(0o755)).unwrap(); }
        let call=|v| dispatch(&s,serde_json::from_value(v).unwrap()).unwrap();
        let e=call(json!({"op":"add_engine","path":bin.join("wine")}));
        let first=call(json!({"op":"create","name":"First","engine_id":e["id"],"dpi":192}));
        let second=call(json!({"op":"create","name":"Second","engine_id":e["id"]}));
        (s,first["id"].as_str().unwrap().into(),second["id"].as_str().unwrap().into())
    }
    #[test]
    fn delete_is_recoverable_and_does_not_affect_other_environments() {
        let (s,first,second)=setup();let prefix=s.prefix(&first).unwrap();
        fs::write(prefix.join("drive_c/save.dat"),"saved game").unwrap();
        s.update(|state| { bottle_mut(state,&first)?.environment.insert("CUSTOM_VALUE".into(),"keep".into());Ok(()) }).unwrap();
        { let _busy=s.operation_lock(&first).unwrap(); assert!(trash(&s,&first).is_err()); }
        trash(&s,&first).unwrap();
        let state=s.read().unwrap();assert_eq!(state.bottles.len(),1);assert_eq!(state.bottles[0].id,second);
        assert_eq!(state.deleted_bottles.len(),1);assert!(s.pair(&first).is_err());
        assert_eq!(fs::read_to_string(prefix.join("drive_c/save.dat")).unwrap(),"saved game");
        let log=fs::read_to_string(s.root.join("logs").join(state.deleted_bottles[0].last_log.as_ref().unwrap())).unwrap();
        assert!(log.contains(prefix.to_str().unwrap()));assert!(!log.contains(s.prefix(&second).unwrap().to_str().unwrap()));
        restore(&s,&first).unwrap();let (b,_)=s.pair(&first).unwrap();
        assert_eq!(b.dpi,Some(192));assert_eq!(b.environment["CUSTOM_VALUE"],"keep");
        assert!(s.read().unwrap().deleted_bottles.is_empty());assert!(purge(&s,&first).is_err());
        fs::remove_dir_all(s.root).unwrap();
    }
    #[test]
    fn purge_removes_only_its_files_and_does_not_follow_host_symlinks() {
        let (s,first,second)=setup();let outside=s.root.join("host-files");fs::create_dir(&outside).unwrap();
        fs::write(outside.join("important.txt"),"keep").unwrap();
        symlink(&outside,s.prefix(&first).unwrap().join("drive_c/host-link")).unwrap();
        let installer=s.root.join("installers").join(&first);fs::create_dir_all(&installer).unwrap();fs::write(installer.join("setup.exe"),"cache").unwrap();
        trash(&s,&first).unwrap();purge(&s,&first).unwrap();
        assert!(!s.prefix(&first).unwrap().exists());assert!(!installer.exists());
        assert!(s.prefix(&second).unwrap().exists());assert!(s.root.join("engine/bin/wine").exists());
        assert_eq!(fs::read_to_string(outside.join("important.txt")).unwrap(),"keep");
        assert!(s.read().unwrap().deleted_bottles.is_empty());
        fs::remove_dir_all(s.root).unwrap();
    }
    #[test]
    fn failed_stop_and_escaping_paths_keep_data_untouched() {
        let (s,first,_)=setup();
        fs::write(s.root.join("engine/bin/wineserver"),"#!/bin/sh\necho failure >&2\nexit 2\n").unwrap();
        assert!(trash(&s,&first).is_err());assert!(s.pair(&first).is_ok());assert!(s.read().unwrap().deleted_bottles.is_empty());
        assert!(trash(&s,"../other").is_err());
        fs::rename(s.root.join("prefixes"),s.root.join("moved-prefixes")).unwrap();
        symlink(s.root.join("moved-prefixes"),s.root.join("prefixes")).unwrap();
        assert!(s.prefix(&first).is_err());
        fs::remove_file(s.root.join("prefixes")).unwrap();fs::remove_dir_all(s.root).unwrap();
    }
}
