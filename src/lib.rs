pub mod win;

use anyhow::Context as _;
use blake2::Digest as _;

pub struct InstallFolder {
    pub dir: std::path::PathBuf,
    pub exe: std::path::PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExeVersion {
    NisaV1_05En, // NISA version 1.05, english/french language
    NisaV1_05Jp, // NISA version 1.05, english/french language
}

impl ExeVersion {
    pub fn relative_path(&self) -> &'static str {
        match self {
            ExeVersion::NisaV1_05En => "bin\\x64\\ed8_3_PC.exe",
            ExeVersion::NisaV1_05Jp => "bin\\x64\\ed8_3_PC_JP.exe",
        }
    }
}

pub fn exe_hash_matches_1_05<T: AsRef<[u8]>>(_data: T) -> bool {
    &blake2::Blake2b512::digest(data.as_ref())[..]
        == hex_literal::hex!("bbce84df472cdeefd60464715aadfd7e35ca6a9107da320ea137b9424fc3009a4ecf55090f7a439a1ec4969df02bd190e46dd102c1f9bfbfa18d0b8babc989ff")
}

pub fn install_folder_seems_valid(installation: &InstallFolder) -> bool {
    installation.dir.is_dir() && installation.exe.is_file()
}

enum Language {
    En,
    Fr,
    Jp,
}

fn identify_language() -> anyhow::Result<Language> {
    let path = &std::path::Path::new("%LocalAppData%\\TrailsOfColdSteel3\\settings.xml");
    let content = std::fs::read_to_string(path).with_context(|| format!("couldn't open settings file {:?}", path.display()))?;
    // Yes this is ugly
    // No I'm not adding an XML lib as a dependency just for this
    if content.contains("<Language>English</Language>") {
        Ok(Language::En)
    } else if content.contains("<Language>Japanese</Language>") {
        Ok(Language::Jp)
    } else if content.contains("<Language>French</Language>") {
        Ok(Language::Fr)
    } else {
        Err(anyhow::Error::msg(format!("settings file {} doesn't seem to contain a <Language> element", path.display())))
    }
}

pub fn identify_version(base_path: &std::path::Path) -> anyhow::Result<ExeVersion> {
    let language = identify_language().context("couldn't identify game language")?;

    let (exe_path, known_hashes) = match language {
        Language::En | Language::Fr => (
            "bin\\x64\\ed8_3_PC.exe",
            &[(
                ExeVersion::NisaV1_05En,
                hex_literal::hex!("bbce84df472cdeefd60464715aadfd7e35ca6a9107da320ea137b9424fc3009a4ecf55090f7a439a1ec4969df02bd190e46dd102c1f9bfbfa18d0b8babc989ff"),
            )],
        ),
        Language::Jp => (
            "bin\\x64\\ed8_3_PC_JP.exe",
            &[(
                ExeVersion::NisaV1_05Jp,
                hex_literal::hex!("2e846b7f10bda3d3ae191d4391b35b879683f73628e9bcb8a93b10448e65ecf22a496e3fe90c06aa140bb75bdfc09965367d4209c6311272e67d9a208104da7d"),
            )],
        ),
    };

    let path = base_path.join(exe_path);
    let data = std::fs::read(&path).with_context(|| format!("couldn't read executable file {}", path.display()))?;
    let hash = blake2::Blake2b512::digest(&data);
    for (exe_version, expected_hash) in known_hashes.iter().copied() {
        if &hash[..] == expected_hash {
            return Ok(exe_version);
        }
    }

    Err(anyhow::Error::msg(format!("Executable file {} hash doesn't match any known one!", path.display())))
}

pub fn find_base_path() -> Option<std::path::PathBuf> {
    let paths = [
        "GOG Games\\ToCS3",
        "ToCS3",
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
    relative_folders.chain(absolute_folders).into_iter().find(|path| path.is_dir())
}

pub fn find_install_folder() -> Option<InstallFolder> {
    let paths = [
        "GOG Games\\ToCS3",
        "ToCS3",
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
        let exe = dir.join("bin\\x64\\ed8_3_PC.exe");
        let i = InstallFolder { dir, exe };
        if install_folder_seems_valid(&i) {
            return Some(i);
        }
    }
    None
}

unsafe fn apply_patch<N: generic_array::ArrayLength<u8>>(
    handle: windows::Win32::Foundation::HANDLE,
    offset: usize,
    expected: &generic_array::GenericArray<u8, N>,
    new: Option<&generic_array::GenericArray<u8, N>>,
) -> anyhow::Result<()> {
    let mut actual: generic_array::GenericArray<u8, N> = generic_array::GenericArray::default();
    win::read_process_memory(handle, offset as *const core::ffi::c_void, &mut actual).context("couldn't read from process")?;
    if &actual == expected {
        if let Some(buf) = new {
            win::write_process_memory(handle, offset as *mut core::ffi::c_void, &buf).context("couldn't write to process")?;
        }
        anyhow::Ok(())
    } else if Some(&actual) == new {
        Err(anyhow::Error::msg("unexpected value: patch may have been applied already!"))
    } else {
        Err(anyhow::Error::msg(format!("unexpected value: expected {expected:?}, got {actual:?}")))
    }
}

pub unsafe fn apply_patches(handle: windows::Win32::Foundation::HANDLE) -> anyhow::Result<()> {
    // check game version
    apply_patch(handle, 0x140825d68, b"ToCSIII PC Version 1.05\0".into(), Some(b"ToCS3 1.05 autoskip mod\0".into())).context("couldn't update game version")?;

    // sets the "skip" flag to 0 at the end of a skipped cutscene
    apply_patch(
        handle,
        0x14032a5b1,
        (&[0xc6, 0x83, 0x3d, 0x3d, 0x00, 0x00, 0x00]).into(),
        Some((&[0xc6, 0x83, 0x3d, 0x3d, 0x00, 0x00, 0x01]).into()),
    )
    .context("couldn't patch cutscene flag setter 0x14032a5b1")?;

    // sets the "skip" flag to 0
    // not sure what the exact trigger here is, but it seems to run at the start of some cutscenes?
    // it also seems like it's always overridden by the above code, but it doesn't hurt to patch it out anyway
    apply_patch(
        handle,
        0x1403dfda3,
        (&[0xc6, 0x80, 0x3d, 0x3d, 0x00, 0x00, 0x00]).into(),
        Some((&[0xc6, 0x80, 0x3d, 0x3d, 0x00, 0x00, 0x01]).into()),
    )
    .context("couldn't patch cutscene flag setter 0x1403dfda3")?;

    // and finally set the "skip" flag itself to 1 once for good measure
    apply_patch(handle, 0x1416c535d, (&[0x00]).into(), Some((&[0x01]).into())).context("couldn't set cutscene skip flag")?;

    anyhow::Ok(())
}

// This struct represents the suspended game process
// It implements drop to automatically close the game when an error occurs.
// If the game patch process was successful, call .resume() to resume the game and close internal handles.
struct SuspendedGameProcess {
    proc_info: windows::Win32::System::Threading::PROCESS_INFORMATION,
    resumed: bool,
}

impl SuspendedGameProcess {
    pub unsafe fn new(exe_path: &std::path::Path) -> std::io::Result<Self> {
        // we take startup info from the current process
        let startup_info = windows::Win32::System::Threading::STARTUPINFOW::default();
        //windows::Win32::System::Threading::GetStartupInfoW(std::ptr::addr_of_mut!(startup_info));
        let name = windows::core::HSTRING::from(exe_path);

        let proc_info = win::create_process_w(
            windows::core::PCWSTR(name.as_ptr()),   // name
            windows::core::PWSTR(0 as *mut u16),    // command_line
            None,                                   // process security attributes
            None,                                   // thread security attributes
            false,                                  // inherit handles?
            win::CREATE_SUSPENDED,                  // creation flags
            None,                                   // environment
            windows::core::PCWSTR(0 as *const u16), // working directory
            &startup_info,
        )?;

        Ok(Self { proc_info, resumed: false })
    }

    pub fn process_handle(&self) -> windows::Win32::Foundation::HANDLE {
        self.proc_info.hProcess
    }

    pub unsafe fn resume(&mut self) -> std::io::Result<u32> {
        let ret = win::resume_thread(self.proc_info.hThread)?;
        self.resumed = true;
        Ok(ret)
    }
}

impl Drop for SuspendedGameProcess {
    fn drop(&mut self) {
        unsafe {
            if !self.resumed {
                _ = win::terminate_process(self.process_handle(), 1);
            }
            _ = win::close_handle(self.proc_info.hProcess);
            _ = win::close_handle(self.proc_info.hThread);
        }
    }
}

pub fn run_and_patch_game(exe_path: &std::path::Path) -> anyhow::Result<()> {
    // Check that we got the right exe
    let exe_data = std::fs::read(exe_path).context("couldn't check exe data")?;
    if !exe_hash_matches_1_05(&exe_data) {
        return Err(anyhow::Error::msg("Unexpected executable. Are you sure this is the right version?"));
    }

    unsafe {
        let mut game = SuspendedGameProcess::new(exe_path).context("couldn't launch suspended game process")?;

        // Obtain information about the text segment pages
        let text_offset = 0x140001000usize;
        let rdata_offset = 0x1407ea000usize;
        let text_mem_info = win::virtual_query_ex(game.process_handle(), Some(text_offset as *const core::ffi::c_void)).context("couldn't query process text pages")?;
        let rdata_mem_info = win::virtual_query_ex(game.process_handle(), Some(rdata_offset as *const core::ffi::c_void)).context("couldn't query process rdata pages")?;

        if text_mem_info.BaseAddress as usize + text_mem_info.RegionSize < 0x1403dfda3 {
            // Are our addresses of interest part of the page region?
            return Err(anyhow::Error::msg("text segment page region is unexpectedly small, bailing out"));
        }
        if rdata_mem_info.BaseAddress as usize + rdata_mem_info.RegionSize < 0x140825d68 {
            return Err(anyhow::Error::msg("rdata segment page region is unexpectedly small, bailing out"));
        }

        // Change memory protection flags
        let rw = win::PAGE_READWRITE;
        let old_text_flags = win::virtual_protect_ex(game.process_handle(), text_mem_info.BaseAddress, text_mem_info.RegionSize, rw).context("couldn't change text memory protection")?;
        let old_rdata_flags = win::virtual_protect_ex(game.process_handle(), rdata_mem_info.BaseAddress, rdata_mem_info.RegionSize, rw).context("couldn't change rdata memory protection")?;

        // apply patches
        apply_patches(game.process_handle()).context("couldn't apply mod patches")?;

        // Change memory protection flags back
        let _ = win::virtual_protect_ex(game.process_handle(), text_mem_info.BaseAddress, text_mem_info.RegionSize, old_text_flags)
            .context("couldn't change text memory protection back to original value")?;
        let _ = win::virtual_protect_ex(game.process_handle(), rdata_mem_info.BaseAddress, rdata_mem_info.RegionSize, old_rdata_flags)
            .context("couldn't change rdata memory protection back to original value")?;

        // Resume main thread, actually starting the game
        game.resume().context("couldn't resume game process after applying patch")?;
    }

    Ok(())
}
