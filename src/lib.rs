pub mod win;

use anyhow::Context as _;
use blake2::Digest as _;

pub struct InstallFolder {
    pub dir: std::path::PathBuf,
    pub exe: std::path::PathBuf,
}

pub fn exe_hash_matches_1_05<T: AsRef<[u8]>>(data: T) -> bool {
    &blake2::Blake2b512::digest(data.as_ref())[..]
        == hex_literal::hex!("bbce84df472cdeefd60464715aadfd7e35ca6a9107da320ea137b9424fc3009a4ecf55090f7a439a1ec4969df02bd190e46dd102c1f9bfbfa18d0b8babc989ff")
}

pub fn install_folder_seems_valid(installation: &InstallFolder) -> bool {
    installation.dir.is_dir()
        && std::fs::read(&installation.exe)
            .map(exe_hash_matches_1_05) // might be too strong of a check
            .unwrap_or(false)
}

pub fn find_install_folder() -> Option<InstallFolder> {
    let paths = [
        "GOG Games\\ToCS3",
        "Program Files (x86)\\Steam\\steamapps\\common\\The Legend of Heroes Trails of Cold Steel III",
        "SteamLibrary\\steamapps\\common\\The Legend of Heroes Trails of Cold Steel III",
    ];
    let absolute_folders = win::get_drives_with_fallback()
        .map(|drv| {
            let drv = std::path::PathBuf::from(format!("{drv}:\\"));
            paths.map(|path| drv.join(path))
        })
        .flatten();
    let relative_folders = ["."].into_iter().map(std::path::PathBuf::from);
    let all_folders = relative_folders.chain(absolute_folders);
    for dir in all_folders {
        let exe = dir.join("bin/x64/ed8_3_PC.exe");
        let i = InstallFolder { dir, exe };
        if install_folder_seems_valid(&i) {
            return Some(i);
        }
    }
    None
}

unsafe fn apply_patch<N: generic_array::ArrayLength<u8>>(
    handle: windows::Win32::Foundation::HANDLE,
    description: &str,
    offset: usize,
    expected: &generic_array::GenericArray<u8, N>,
    new: Option<&generic_array::GenericArray<u8, N>>,
) -> anyhow::Result<()> {
    let mut actual: generic_array::GenericArray<u8, N> = generic_array::GenericArray::default();
    win::read_process_memory(handle, offset as *const core::ffi::c_void, &mut actual).with_context(|| format!("couldn't read {description} from launched process"))?;
    if &actual == expected {
        if let Some(buf) = new {
            win::write_process_memory(handle, offset as *mut core::ffi::c_void, &buf).with_context(|| format!("couldn't patch {description}"))?;
        }
        anyhow::Ok(())
    } else if Some(&actual) == new {
        Err(anyhow::Error::msg(format!("unexpected {description}: patch may have been applied already!")))
    } else {
        Err(anyhow::Error::msg(format!("unexpected {description}: expected {expected:?}, got {actual:?}")))
    }
}

pub unsafe fn try_patch(handle: windows::Win32::Foundation::HANDLE) -> anyhow::Result<()> {
    // check game version
    apply_patch(handle, "game version", 0x140825d68, b"ToCSIII PC Version 1.05\0".into(), Some(b"ToCS3 1.05 autoskip mod\0".into()))?;

    // sets the "skip" flag to 0 at the end of a skipped cutscene
    apply_patch(
        handle,
        "reset cutscene skip flag #1",
        0x14032a5b1,
        (&[0xc6, 0x83, 0x3d, 0x3d, 0x00, 0x00, 0x00]).into(),
        Some((&[0xc6, 0x83, 0x3d, 0x3d, 0x00, 0x00, 0x01]).into()),
    )?;

    // sets the "skip" flag to 0
    // not sure what the exact trigger here is, but it seems to run at the start of some cutscenes?
    // it also seems like it's always overridden by the above code, but it doesn't hurt to patch it out anyway
    apply_patch(
        handle,
        "reset cutscene skip flag #2",
        0x1403dfda3,
        (&[0xc6, 0x80, 0x3d, 0x3d, 0x00, 0x00, 0x00]).into(),
        Some((&[0xc6, 0x80, 0x3d, 0x3d, 0x00, 0x00, 0x01]).into()),
    )?;

    // and finally set the "skip" flag itself to 1 once for good measure
    apply_patch(handle, "set cutscene skip flag to 1", 0x1416c535d, (&[0x00]).into(), Some((&[0x01]).into()))?;

    anyhow::Ok(())
}
