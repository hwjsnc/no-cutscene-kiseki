use crate::win;

use anyhow::Context as _;
use blake2::Digest as _;

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

    pub fn expected_blake2b_hash(&self) -> [u8; 64] {
        match self {
            ExeVersion::NisaV1_05En => hex_literal::hex!("bbce84df472cdeefd60464715aadfd7e35ca6a9107da320ea137b9424fc3009a4ecf55090f7a439a1ec4969df02bd190e46dd102c1f9bfbfa18d0b8babc989ff"),
            ExeVersion::NisaV1_05Jp => hex_literal::hex!("2e846b7f10bda3d3ae191d4391b35b879683f73628e9bcb8a93b10448e65ecf22a496e3fe90c06aa140bb75bdfc09965367d4209c6311272e67d9a208104da7d"),
        }
    }
}

enum Language {
    En,
    Fr,
    Jp,
}

fn identify_language() -> anyhow::Result<Language> {
    let local_app_data = win::get_local_app_data().context("couldn't find %LocalAppData%")?;
    println!("{}", local_app_data.display());
    let path = local_app_data.join("TrailsOfColdSteel3\\settings.xml");
    println!("{}", path.display());
    let content = std::fs::read_to_string(&path).with_context(|| format!("couldn't open settings file {:?}", path.display()))?;
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

    let (exe_path, known_versions) = match language {
        Language::En | Language::Fr => ("bin\\x64\\ed8_3_PC.exe", &[ExeVersion::NisaV1_05En]),
        Language::Jp => ("bin\\x64\\ed8_3_PC_JP.exe", &[ExeVersion::NisaV1_05Jp]),
    };

    let path = base_path.join(exe_path);
    let data = std::fs::read(&path).with_context(|| format!("couldn't read executable file {}", path.display()))?;
    let hash = blake2::Blake2b512::digest(&data);
    for version in known_versions.into_iter().copied() {
        if &hash[..] == version.expected_blake2b_hash() {
            return Ok(version);
        }
    }

    Err(anyhow::Error::msg(format!("Executable file {} hash doesn't match any known one!", path.display())))
}

pub fn find_install_path() -> Option<std::path::PathBuf> {
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
    relative_folders.chain(absolute_folders).into_iter().find(|path| path.is_dir() && path.join("bin\\x64").is_dir())
}

struct Patch<'a> {
    name: &'a str,
    offset: usize,
    expected: &'a [u8],
    replacement: Option<&'a [u8]>,
}

unsafe fn apply_patch(
    handle: windows::Win32::Foundation::HANDLE,
    patch: &Patch,
    buf: &mut [u8], // scratch buffer, must have the same length as expected target data
) -> anyhow::Result<()> {
    // Check that we have enough memory
    assert_eq!(buf.len(), patch.expected.len());
    if let Some(new) = patch.replacement {
        assert_eq!(new.len(), patch.expected.len());
    }
    win::read_process_memory(handle, patch.offset as *const core::ffi::c_void, buf).context("couldn't read from process")?;
    if buf == patch.expected {
        if let Some(new) = patch.replacement {
            win::write_process_memory(handle, patch.offset as *mut core::ffi::c_void, new).context("couldn't write to process")?;
        }
        anyhow::Ok(())
    } else if Some(buf as &[u8]) == patch.replacement {
        Err(anyhow::Error::msg("unexpected value: patch may have been applied already!"))
    } else {
        Err(anyhow::Error::msg(format!(
            "unexpected values at offset {:x}: expected {:?}, got {:?}",
            patch.offset, patch.expected, buf
        )))
    }
}

unsafe fn apply_patches(
    handle: windows::Win32::Foundation::HANDLE,
    patches: &[Patch],
    buf: &mut [u8], // scratch buffer, must be at least as long as the largest patch data
) -> anyhow::Result<()> {
    // before patching anything, confirm that all target data is correct
    for patch in patches {
        let p = Patch {
            name: patch.name,
            offset: patch.offset,
            expected: patch.expected,
            replacement: None,
        };
        assert!(buf.len() >= patch.expected.len());
        apply_patch(handle, &p, &mut buf[..patch.expected.len()]).with_context(|| format!("couldn't apply patch {}", patch.name))?;
    }
    // now actually apply the patches
    for patch in patches {
        apply_patch(handle, patch, &mut buf[..patch.expected.len()]).with_context(|| format!("couldn't apply patch {}", patch.name))?;
    }
    Ok(())
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

struct Offsets {
    text: std::ops::Range<usize>,
    rdata: std::ops::Range<usize>,
    data: std::ops::Range<usize>,
}

pub fn run_and_patch(path: &std::path::Path, version: ExeVersion) -> anyhow::Result<()> {
    // check that we got the right exe
    // Do not delete this check! Right now we determine the game version by checking the hash, so we can only pass the correct version here.
    // In the future, I may add such an option and when that happens, this check is vital to avoid errors when the user selects a wrong version!
    let exe_path = path.join(version.relative_path());
    let exe_data = std::fs::read(&exe_path).context("couldn't check exe data")?;
    let exe_hash = blake2::Blake2b512::digest(&exe_data);
    if &exe_hash[..] != version.expected_blake2b_hash() {
        return Err(anyhow::Error::msg("Unexpected executable. Are you sure this is the right version?"));
    }

    let off = match version {
        ExeVersion::NisaV1_05En => Offsets {
            text: 0x140001000..0x1407e97ff,
            rdata: 0x1407ea000..0x140a1b7ff,
            data: 0x140a1c000..0x141d73300,
        },
        ExeVersion::NisaV1_05Jp => Offsets {
            text: 0x140001000..0x1407dc7ff,
            rdata: 0x1407dd000..0x140a05dff,
            data: 0x140a06000..0x141d5d140,
        },
    };

    let patches = match version {
        ExeVersion::NisaV1_05En => &[
            Patch {
                name: "game version",
                offset: 0x140825d68,
                expected: b"ToCSIII PC Version 1.05\0",
                replacement: Some(b"ToCS3 1.05 autoskip mod\0"),
            },
            Patch {
                name: "skip flag setter",
                offset: 0x14032a5b1,
                expected: &[0xc6, 0x83, 0x3d, 0x3d, 0x00, 0x00, 0x00],          // MOV byte ptr [RBX + 0x3d3d],0x00
                replacement: Some(&[0xc6, 0x83, 0x3d, 0x3d, 0x00, 0x00, 0x01]), // MOV byte ptr [RBX + 0x3d3d],0x01
            },
            Patch {
                name: "skip flag setter",
                offset: 0x1403dfda3,
                expected: &[0xc6, 0x80, 0x3d, 0x3d, 0x00, 0x00, 0x00],          // MOV byte ptr [RAX + 0x3d3d],0x00
                replacement: Some(&[0xc6, 0x80, 0x3d, 0x3d, 0x00, 0x00, 0x01]), // MOV byte ptr [RAX + 0x3d3d],0x01
            },
            Patch {
                name: "skip flag",
                offset: 0x1416c535d,
                expected: &[0x00],
                replacement: Some(&[0x01]),
            },
        ],
        ExeVersion::NisaV1_05Jp => {
            return Err(anyhow::Error::msg("TODO: japanese is not supported yet"));
        }
    };

    // check patches
    for patch in patches {
        assert!(
            (off.text.contains(&patch.offset) && off.text.contains(&(patch.offset + patch.expected.len())))
                || (off.data.contains(&patch.offset) && off.data.contains(&(patch.offset + patch.expected.len())))
                || (off.rdata.contains(&patch.offset) && off.rdata.contains(&(patch.offset + patch.expected.len())))
        );
    }

    let mut buf = [0u8; 24]; // must be at least as long as the longest patch

    unsafe {
        // launch game, suspended
        let mut game = SuspendedGameProcess::new(&exe_path).context("couldn't launch suspended game process")?;
        let handle = game.process_handle();

        // Obtain information about segment pages
        let text_mem_info = win::virtual_query_ex(handle, Some(off.text.start as *const std::ffi::c_void)).context("couldn't query process text pages")?;
        let rdata_mem_info = win::virtual_query_ex(handle, Some(off.rdata.start as *const std::ffi::c_void)).context("couldn't query process rdata pages")?;
        // No need to update data segment, it should be R/W anyway

        if text_mem_info.BaseAddress as usize + text_mem_info.RegionSize < off.text.end {
            // Are our addresses of interest part of the page region?
            return Err(anyhow::Error::msg("text segment page region is unexpectedly small, bailing out"));
        }
        if rdata_mem_info.BaseAddress as usize + rdata_mem_info.RegionSize < off.text.end {
            return Err(anyhow::Error::msg("rdata segment page region is unexpectedly small, bailing out"));
        }

        // Change memory protection flags
        let rw = win::PAGE_READWRITE;
        let old_text_flags = win::virtual_protect_ex(handle, text_mem_info.BaseAddress, text_mem_info.RegionSize, rw).context("couldn't change text memory protection")?;
        let old_rdata_flags = win::virtual_protect_ex(handle, rdata_mem_info.BaseAddress, rdata_mem_info.RegionSize, rw).context("couldn't change rdata memory protection")?;

        // apply patches
        apply_patches(handle, patches, &mut buf[..]).context("couldn't patch game")?;

        // Change memory protection flags back
        let _ = win::virtual_protect_ex(handle, text_mem_info.BaseAddress, text_mem_info.RegionSize, old_text_flags).context("couldn't change text memory protection back to original value")?;
        let _ = win::virtual_protect_ex(handle, rdata_mem_info.BaseAddress, rdata_mem_info.RegionSize, old_rdata_flags).context("couldn't change rdata memory protection back to original value")?;

        // Resume main thread, actually starting the game
        game.resume().context("couldn't resume game process after applying patch")?;
    }

    Ok(())
}
