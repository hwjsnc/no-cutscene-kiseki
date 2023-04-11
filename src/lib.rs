use anyhow::Context as _;
use process_memory::CopyAddress as _;
use process_memory::PutAddress as _;

fn apply_patch<N: generic_array::ArrayLength<u8>>(
    handle: &process_memory::ProcessHandle,
    description: &str,
    offset: usize,
    expected: &generic_array::GenericArray<u8, N>,
    new: Option<&generic_array::GenericArray<u8, N>>,
) -> anyhow::Result<()> {
    let mut actual: generic_array::GenericArray<u8, N> = generic_array::GenericArray::default();
    handle.copy_address(offset, &mut actual).with_context(|| format!("checking {description}"))?;
    if &actual == expected {
        if let Some(x) = new {
            // TODO: fails on linux, probably need to ptrace the process first
            handle.put_address(offset, x).with_context(|| format!("patching {description}"))?;
        }
        anyhow::Ok(())
    } else if Some(&actual) == new {
        Err(anyhow::Error::msg(format!("unexpected {description}: patch may have been applied already!")))
    } else {
        Err(anyhow::Error::msg(format!("unexpected {description}: expected {expected:?}, got {actual:?}")))
    }
}

pub fn try_patch(handle: &process_memory::ProcessHandle) -> anyhow::Result<()> {
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

pub fn message<S: AsRef<str>>(msg: S) {
    let stdout_is_a_terminal = atty::is(atty::Stream::Stdout);
    if stdout_is_a_terminal {
        println!("{}", msg.as_ref());
    } else {
        fltk::dialog::message_default(msg.as_ref());
    }
}

pub fn alert<S: AsRef<str>>(msg: S) {
    let stderr_is_a_terminal = atty::is(atty::Stream::Stderr);
    if stderr_is_a_terminal {
        eprintln!("{}", msg.as_ref());
    } else {
        fltk::dialog::alert_default(msg.as_ref());
    }
}

#[cfg(target_os = "windows")]
mod win {
    // various rusty wrappers around the win32 API

    pub unsafe fn virtual_query_ex(handle: windows::Win32::Foundation::HANDLE, address: Option<*const core::ffi::c_void>) -> std::io::Result<windows::Win32::System::Memory::MEMORY_BASIC_INFORMATION> {
        let mut mem_info = windows::Win32::System::Memory::MEMORY_BASIC_INFORMATION::default();
        let mem_info_size = std::mem::size_of::<windows::Win32::System::Memory::MEMORY_BASIC_INFORMATION>();
        let ret = windows::Win32::System::Memory::VirtualQueryEx(handle, address, std::ptr::addr_of_mut!(mem_info), mem_info_size);
        if ret == mem_info_size {
            Ok(mem_info)
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub unsafe fn virtual_protect_ex(
        handle: windows::Win32::Foundation::HANDLE,
        address: *const core::ffi::c_void,
        size: usize,
        new_flags: windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS,
    ) -> std::io::Result<windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS> {
        let mut old_flags = windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS::default();
        let ret = windows::Win32::System::Memory::VirtualProtectEx(handle, address, size, new_flags, std::ptr::addr_of_mut!(old_flags));
        if ret.0 != 0 {
            Ok(old_flags)
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub unsafe fn open_process(
        rights: windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS,
        inherit_handle: bool,
        process: u32,
    ) -> Result<windows::Win32::Foundation::HANDLE, windows::core::Error> {
        windows::Win32::System::Threading::OpenProcess(rights, inherit_handle, process)
    }

    pub unsafe fn read_process_memory(handle: windows::Win32::Foundation::HANDLE, source: *const core::ffi::c_void, target: &mut [u8]) -> std::io::Result<usize> {
        let mut number_of_bytes_read = 0;
        let ret = windows::Win32::System::Diagnostics::Debug::ReadProcessMemory(
            handle,
            source,
            target.as_mut_ptr() as *mut core::ffi::c_void,
            target.len(),
            Some(std::ptr::addr_of_mut!(number_of_bytes_read)),
        );
        if ret.0 != 0 {
            Ok(number_of_bytes_read)
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub unsafe fn write_process_memory(handle: windows::Win32::Foundation::HANDLE, target: *mut core::ffi::c_void, source: &[u8]) -> std::io::Result<usize> {
        let mut number_of_bytes_written = 0;
        let ret = windows::Win32::System::Diagnostics::Debug::WriteProcessMemory(
            handle,
            target,
            source.as_ptr() as *const core::ffi::c_void,
            source.len(),
            Some(std::ptr::addr_of_mut!(number_of_bytes_written)),
        );
        if ret.0 != 0 {
            Ok(number_of_bytes_written)
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub unsafe fn create_process_w(
        name: windows::core::PCWSTR,
        command_line: windows::core::PWSTR,
        process_attributes: Option<*const windows::Win32::Security::SECURITY_ATTRIBUTES>,
        thread_attributes: Option<*const windows::Win32::Security::SECURITY_ATTRIBUTES>,
        inherit_handles: bool,
        creation_flags: windows::Win32::System::Threading::PROCESS_CREATION_FLAGS,
        environment: Option<*const core::ffi::c_void>,
        current_directory: windows::core::PCWSTR,
        startup_info: &windows::Win32::System::Threading::STARTUPINFOW,
    ) -> std::io::Result<windows::Win32::System::Threading::PROCESS_INFORMATION>
where {
        let mut process_info = windows::Win32::System::Threading::PROCESS_INFORMATION::default();
        let ret = windows::Win32::System::Threading::CreateProcessW(
            name,
            command_line,
            process_attributes,
            thread_attributes,
            inherit_handles,
            creation_flags,
            environment,
            current_directory,
            startup_info,
            // name: full path and file name (or partial file name) windows::core::PCWSTR
            // command line: can be null, in which case name is the command line
            // process & thread security attributes: whether handles can be inherited by child processes; if null: no
            // inherit_handles: called process inherits handles from calling process - probably want no?
            // creation flags: e.g. create suspended process
            // environment: env, should probably be null
            // current directory: if null, caller's
            // startupinfo: various startup options, most of these can probably be 0
            // processinfo: returns info about the new process; handles must be closed with CloseHandle
            std::ptr::addr_of_mut!(process_info),
        );
        if ret.0 != 0 {
            Ok(process_info)
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub struct ProcessCreationOptions {
        name: windows::core::HSTRING,
        flags: windows::Win32::System::Threading::PROCESS_CREATION_FLAGS,
    }

    impl ProcessCreationOptions {
        pub fn new() -> Self {
            Self {
                name: windows::core::HSTRING::new(),
                flags: windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(0),
            }
        }

        pub fn with_name(mut self, path: &std::path::Path) -> Self {
            self.name = path.into();
            self
        }

        pub fn with_flags(mut self, flags: windows::Win32::System::Threading::PROCESS_CREATION_FLAGS) -> Self {
            self.flags = flags;
            self
        }
    }

    pub unsafe fn create_process_w_simplified(opts: &ProcessCreationOptions) -> std::io::Result<windows::Win32::System::Threading::PROCESS_INFORMATION> {
        // we let the callee inherit our own startup info
        let mut startup_info = windows::Win32::System::Threading::STARTUPINFOW::default();
        windows::Win32::System::Threading::GetStartupInfoW(std::ptr::addr_of_mut!(startup_info));

        create_process_w(
            windows::core::PCWSTR(opts.name.as_ptr()), // name
            windows::core::PWSTR(0 as *mut u16),       // command line (null)
            None,                                      // process security attributes
            None,                                      // thread security attributes
            false,                                     // inherit handles?
            opts.flags,                                // process creation flags
            None,                                      // environment
            windows::core::PCWSTR(0 as *const u16),    // working directory
            &startup_info,                             // startup info
        )
    }

    pub unsafe fn close_handle(handle: windows::Win32::Foundation::HANDLE) -> std::io::Result<()> {
        let ret = windows::Win32::Foundation::CloseHandle(handle);
        if ret.0 != 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}
