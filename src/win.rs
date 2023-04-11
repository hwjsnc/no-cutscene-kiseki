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

pub unsafe fn open_process(rights: windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS, inherit_handle: bool, process: u32) -> Result<windows::Win32::Foundation::HANDLE, windows::core::Error> {
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

pub fn get_drives_with_fallback() -> impl Iterator<Item = char> {
    let mut drives = unsafe { windows::Win32::Storage::FileSystem::GetLogicalDrives() };
    if drives == 0 {
        drives = 0x04; // 3rd-least significant bit is set, corresponds to C drive
    }
    ('A'..='Z').filter(move |d| {
        let n = *d as u32 - 'A' as u32;
        (drives >> n) & 1 != 0
    })
}
