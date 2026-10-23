use crate::local;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct Location {
    pub wine: PathBuf,
    pub server: PathBuf,
    pub root: Option<PathBuf>,
    pub architecture: String,
    pub backend: String,
    pub supports_x86_32: Option<bool>,
}

// Mach-O headers are inspected directly: the installed app does not require Xcode/lipo.
pub fn architectures(header: &[u8]) -> Vec<String> {
    fn label(cpu: u32) -> Option<String> {
        match cpu { 0x0100000c => Some("arm64".into()), 0x01000007 => Some("x86_64".into()), _ => None }
    }
    fn word(b: &[u8], at: usize, little: bool) -> Option<u32> {
        let bytes: [u8; 4] = b.get(at..at+4)?.try_into().ok()?;
        Some(if little { u32::from_le_bytes(bytes) } else { u32::from_be_bytes(bytes) })
    }
    let mut result = Vec::new();
    match header.get(..4) {
        Some([0xcf, 0xfa, 0xed, 0xfe]) | Some([0xfe, 0xed, 0xfa, 0xcf]) => {
            let little = header[0] == 0xcf;
            if let Some(arch) = word(header, 4, little).and_then(label) { result.push(arch); }
        }
        Some([0xca, 0xfe, 0xba, 0xbe]) | Some([0xca, 0xfe, 0xba, 0xbf]) |
        Some([0xbe, 0xba, 0xfe, 0xca]) | Some([0xbf, 0xba, 0xfe, 0xca]) => {
            let little = header[0] != 0xca;
            let stride = if header[3] == 0xbf || header[0] == 0xbf { 32 } else { 20 };
            let count = word(header, 4, little).unwrap_or(0);
            if count > 32 { return result; }
            for index in 0..count as usize {
                if let Some(arch) = word(header, 8+index*stride, little).and_then(label) {
                    if !result.contains(&arch) { result.push(arch); }
                }
            }
        }
        _ => {}
    }
    result
}

pub fn file_architectures(path: &Path) -> Result<Vec<String>, String> {
    let mut header = Vec::new();
    File::open(path).map_err(|e| e.to_string())?.take(4096).read_to_end(&mut header).map_err(|e| e.to_string())?;
    Ok(architectures(&header))
}

pub fn locate(path: &str) -> Result<Location, String> {
    let p = PathBuf::from(path);
    if !p.is_absolute() { return Err(local("Укажите абсолютный путь к движку", "Specify an absolute engine path").into()); }
    let candidates = [p.clone(), p.join("bin/wine"), p.join("bin/wine64"), p.join("dist/bin/wine"),
        p.join("Contents/Resources/wine/bin/wine"), p.join("Contents/Resources/wine/bin/wine64"),
        p.join("Contents/Resources/bin/wine"), p.join("Contents/SharedSupport/runtime/dist/bin/wine")];
    let chosen = candidates.iter().find(|p| p.is_file()).ok_or(local("Выберите Wine.app, папку runtime или исполняемый файл wine", "Choose Wine.app, a runtime folder, or the wine executable"))?;
    // Preserve bin/wine's path, including a symlink into an intact signed loader bundle.
    // Resolving the loader and looking beside it would lose the matching wineserver.
    let parent = fs::canonicalize(chosen.parent().ok_or(local("Не найдена папка Wine", "Wine folder not found"))?).map_err(|e| e.to_string())?;
    let wine = parent.join(chosen.file_name().ok_or(local("Некорректный путь Wine", "Invalid Wine path"))?);
    let root = wine.ancestors().skip(1).take(8).find(|p| p.join("bin/wineserver").is_file() && p.join("lib/wine").is_dir()).map(Path::to_path_buf);
    let server = root.as_ref().map(|p| p.join("bin/wineserver")).unwrap_or(parent.join("wineserver"));
    if !server.is_file() { return Err(local("Не найден wineserver из той же сборки", "The matching wineserver was not found").into()); }
    let arches = file_architectures(&wine)?;
    let server_arches = file_architectures(&server)?;
    let arm = arches.iter().any(|s| s == "arm64");
    if arm && !server_arches.iter().any(|s| s == "arm64") { return Err(local("У нативного Wine должен быть нативный ARM64 wineserver", "A native Wine engine requires a native ARM64 wineserver").into()); }
    let architecture: String = if arm { "arm64" } else if arches.iter().any(|s| s == "x86_64") { "x86_64" } else { "unknown" }.into();
    let fex = arm && root.as_ref().map(|p| p.join("lib/wine/aarch64-windows/libarm64ecfex.dll").is_file()).unwrap_or(false);
    let backend = if fex { "arm64-fex" } else if arm { "arm64" } else if arches.iter().any(|s| s == "x86_64") { "rosetta" } else { "unknown" }.into();
    let declared_x86_32 = root.as_ref().and_then(|p| fs::read(p.join("vela-runtime.json")).ok())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("supports_x86_32").and_then(|v| v.as_bool()));
    // A 64-bit Mac executable alone does not establish 32-bit Windows support.
    // Respect explicit runtime restrictions before inspecting Intel WoW64 files.
    let supports_x86_32 = declared_x86_32.or_else(|| {
        let root = root.as_ref()?;
        if architecture != "x86_64" { return None; }
        let modules = root.join("lib/wine");
        let present = pe_machine(&modules.join("i386-windows/ntdll.dll")) == Some(0x014c)
            && ["ntdll.dll", "wow64.dll", "wow64cpu.dll", "wow64win.dll"].iter()
                .all(|name| pe_machine(&modules.join("x86_64-windows").join(name)) == Some(0x8664));
        present.then_some(true)
    });
    Ok(Location { wine, server, root, architecture, backend, supports_x86_32 })
}

// Read only the DOS/PE headers; executable contents are never evaluated here.
pub fn pe_machine(path: &Path) -> Option<u16> {
    use std::io::{Seek, SeekFrom};
    let mut f = File::open(path).ok()?;
    let mut dos = [0u8; 64]; f.read_exact(&mut dos).ok()?;
    if &dos[..2] != b"MZ" { return None; }
    let offset = u32::from_le_bytes(dos[60..64].try_into().ok()?) as u64;
    if offset > 16 * 1024 * 1024 { return None; }
    f.seek(SeekFrom::Start(offset)).ok()?;
    let mut pe = [0u8; 6]; f.read_exact(&mut pe).ok()?;
    if &pe[..4] != b"PE\0\0" { return None; }
    Some(u16::from_le_bytes([pe[4], pe[5]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_support_is_separate_from_mac_architecture_and_respects_runtime_policy() {
        let root = std::env::temp_dir().join(format!("vela-wow64-layout-{}", crate::uid()));
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("lib/wine/i386-windows")).unwrap();
        fs::create_dir_all(root.join("lib/wine/x86_64-windows")).unwrap();
        let mut macho=vec![0xcf,0xfa,0xed,0xfe];macho.extend(0x01000007u32.to_le_bytes());
        for name in ["wine","wineserver"] { fs::write(root.join("bin").join(name),&macho).unwrap(); }
        assert_eq!(locate(root.to_str().unwrap()).unwrap().supports_x86_32,None);
        let mut pe=vec![0;134];pe[..2].copy_from_slice(b"MZ");pe[60..64].copy_from_slice(&128u32.to_le_bytes());pe[128..132].copy_from_slice(b"PE\0\0");
        pe[132..134].copy_from_slice(&0x014cu16.to_le_bytes());fs::write(root.join("lib/wine/i386-windows/ntdll.dll"),&pe).unwrap();
        pe[132..134].copy_from_slice(&0x8664u16.to_le_bytes());
        for name in ["ntdll.dll","wow64.dll","wow64cpu.dll","wow64win.dll"] { fs::write(root.join("lib/wine/x86_64-windows").join(name),&pe).unwrap(); }
        assert_eq!(locate(root.to_str().unwrap()).unwrap().supports_x86_32,Some(true));
        fs::write(root.join("lib/wine/x86_64-windows/wow64cpu.dll"),b"not a Windows module").unwrap();
        assert_eq!(locate(root.to_str().unwrap()).unwrap().supports_x86_32,None);
        fs::write(root.join("lib/wine/x86_64-windows/wow64cpu.dll"),&pe).unwrap();
        fs::write(root.join("vela-runtime.json"),r#"{"supports_x86_32":false}"#).unwrap();
        assert_eq!(locate(root.to_str().unwrap()).unwrap().supports_x86_32,Some(false));
        fs::remove_file(root.join("vela-runtime.json")).unwrap();
        macho[4..8].copy_from_slice(&0x0100000cu32.to_le_bytes());
        for name in ["wine","wineserver"] { fs::write(root.join("bin").join(name),&macho).unwrap(); }
        assert_eq!(locate(root.to_str().unwrap()).unwrap().supports_x86_32,None);
        fs::remove_dir_all(root).unwrap();
    }
    #[test] fn windows_executable_bitness_is_read_from_pe_header() {
        let path = std::env::temp_dir().join(format!("vela-pe-{}", crate::uid()));
        let mut file = vec![0u8; 134];
        file[..2].copy_from_slice(b"MZ"); file[60..64].copy_from_slice(&128u32.to_le_bytes());
        file[128..132].copy_from_slice(b"PE\0\0");
        for machine in [0x014cu16, 0x8664, 0xaa64] {
            file[132..134].copy_from_slice(&machine.to_le_bytes()); fs::write(&path, &file).unwrap();
            assert_eq!(pe_machine(&path), Some(machine));
        }
        file[128] = 0; fs::write(&path, &file).unwrap(); assert_eq!(pe_machine(&path), None);
        file[60..64].copy_from_slice(&u32::MAX.to_le_bytes()); fs::write(&path, &file).unwrap();
        assert_eq!(pe_machine(&path), None); fs::remove_file(path).unwrap();
    }
    #[test] fn thin_arm_and_intel_are_distinguished() {
        let mut header = vec![0xcf, 0xfa, 0xed, 0xfe];
        header.extend(0x0100000cu32.to_le_bytes());
        assert_eq!(architectures(&header), vec!["arm64"]);
        header[4..8].copy_from_slice(&0x01000007u32.to_le_bytes());
        assert_eq!(architectures(&header), vec!["x86_64"]);
    }
    #[test] fn universal_and_malformed_headers_are_handled() {
        let mut header = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2];
        for cpu in [0x01000007u32, 0x0100000c] { header.extend(cpu.to_be_bytes()); header.extend([0;16]); }
        assert_eq!(architectures(&header), vec!["x86_64", "arm64"]);
        assert!(architectures(b"#!/bin/sh").is_empty());
        header[4..8].copy_from_slice(&999u32.to_be_bytes());
        assert!(architectures(&header).is_empty());
    }
    #[test] fn signed_loader_symlink_keeps_its_matching_server() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("vela-loader-layout-{}", crate::uid()));
        let loader = root.join("lib/wine/aarch64-unix/wine.app/Contents/MacOS/wine");
        fs::create_dir_all(loader.parent().unwrap()).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("lib/wine/aarch64-windows")).unwrap();
        let mut header = vec![0xcf, 0xfa, 0xed, 0xfe]; header.extend(0x0100000cu32.to_le_bytes());
        fs::write(&loader, &header).unwrap(); fs::write(root.join("bin/wineserver"), &header).unwrap();
        fs::write(root.join("lib/wine/aarch64-windows/libarm64ecfex.dll"), []).unwrap();
        symlink("../lib/wine/aarch64-unix/wine.app/Contents/MacOS/wine", root.join("bin/wine")).unwrap();
        let found = locate(root.to_str().unwrap()).unwrap();
        assert!(found.wine.is_symlink()); assert_eq!(found.server, fs::canonicalize(root.join("bin/wineserver")).unwrap());
        assert_eq!(found.backend, "arm64-fex");
        fs::remove_dir_all(root).unwrap();
    }
}
