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
        name,                                 // full path and file name (or partial file name), can be null
        command_line,                         // can be null
        process_attributes,                   // can handles be inherited? if null: no
        thread_attributes,                    // same
        inherit_handles,                      // child process inherits handles from calling process
        creation_flags,                       // creation flags, e.g. suspended process
        environment,                          // environment, as a string of key=value lines
        current_directory,                    // if null, caller's
        startup_info,                         // processinfo: various startup options, can be fully zero
        std::ptr::addr_of_mut!(process_info), // processinfo: returns info about the new process; handles must be closed with CloseHandle
    );
    if ret.0 != 0 {
        Ok(process_info)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub unsafe fn resume_thread(handle: windows::Win32::Foundation::HANDLE) -> std::io::Result<u32> {
    let ret = windows::Win32::System::Threading::ResumeThread(handle);
    if ret == -1_i32 as u32 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(ret)
    }
}

pub unsafe fn terminate_process(handle: windows::Win32::Foundation::HANDLE, exit_code: u32) -> std::io::Result<()> {
    let ret = windows::Win32::System::Threading::TerminateProcess(handle, exit_code);
    if ret.0 != 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
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

// process handle rights
pub use windows::Win32::System::Threading::PROCESS_ALL_ACCESS;
pub use windows::Win32::System::Threading::PROCESS_QUERY_INFORMATION;
pub use windows::Win32::System::Threading::PROCESS_VM_OPERATION;
pub use windows::Win32::System::Threading::PROCESS_VM_READ;
pub use windows::Win32::System::Threading::PROCESS_VM_WRITE;

// process creation flags (we just need one)
pub use windows::Win32::System::Threading::CREATE_SUSPENDED;

// page protection flags (we just need one)
pub use windows::Win32::System::Memory::PAGE_READWRITE;
