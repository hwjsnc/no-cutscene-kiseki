use common::{find_install_folder, install_folder_seems_valid, try_patch, InstallFolder};

use anyhow::Context as _;
use fltk::prelude::GroupExt as _;
use fltk::prelude::InputExt as _;
use fltk::prelude::WidgetBase as _;
use fltk::prelude::WidgetExt as _;
use process_memory::TryIntoProcessHandle as _;

unsafe fn query_memory(handle: windows::Win32::Foundation::HANDLE, address: Option<*const core::ffi::c_void>) -> Result<windows::Win32::System::Memory::MEMORY_BASIC_INFORMATION, std::io::Error> {
    let mut mem_info = windows::Win32::System::Memory::MEMORY_BASIC_INFORMATION::default();
    let ret = windows::Win32::System::Memory::VirtualQueryEx(
        handle,
        address,
        std::ptr::addr_of_mut!(mem_info),
        std::mem::size_of::<windows::Win32::System::Memory::MEMORY_BASIC_INFORMATION>(),
    );
    if ret == std::mem::size_of::<windows::Win32::System::Memory::MEMORY_BASIC_INFORMATION>() {
        Ok(mem_info)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

unsafe fn change_memory_protect(
    handle: windows::Win32::Foundation::HANDLE,
    address: *const core::ffi::c_void,
    size: usize,
    new_flags: windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS,
) -> Result<windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS, std::io::Error> {
    let mut old_flags = windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS::default();
    let ret = windows::Win32::System::Memory::VirtualProtectEx(handle, address, size, new_flags, std::ptr::addr_of_mut!(old_flags));
    if ret.0 != 0 {
        Ok(old_flags)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn launch_and_patch_game(exe: &std::path::Path) -> anyhow::Result<()> {
    let child = std::process::Command::new(exe).spawn().with_context(|| format!("couldn't launch {}", exe.display()))?;

    // wait 10 ms for the game to start up
    std::thread::sleep(std::time::Duration::from_millis(10));

    // get handle
    let pm_handle = process_memory::Pid::try_from(child.id())
        .map_err(anyhow::Error::msg)
        .and_then(|pid| pid.try_into_process_handle().map_err(anyhow::Error::msg))
        .context("couldn't get game process handle for process-memory")?;

    // get handle for VirtualQueryEx - could probably use the same if not for api version differences
    let handle = unsafe {
        windows::Win32::System::Threading::OpenProcess(
            windows::Win32::System::Threading::PROCESS_QUERY_INFORMATION
                | windows::Win32::System::Threading::PROCESS_VM_OPERATION
                | windows::Win32::System::Threading::PROCESS_VM_READ
                | windows::Win32::System::Threading::PROCESS_VM_WRITE,
            false,
            child.id(),
        )
        .context("couldn't get game process handle for changing memory protect")?
    };

    // query text segment and change memory protection to rwx
    let text_segment_offset = 0x140001000usize;
    let mem_info = unsafe { query_memory(handle, Some(text_segment_offset as *const core::ffi::c_void)).context("querying process memory pages")? };

    if mem_info.BaseAddress as usize + mem_info.RegionSize < 0x1403dfda3 {
        return Err(anyhow::Error::msg("text segment page region is unexpectedly small, bailing out"));
    }

    let rwx = windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS(0x40);
    let oldflags = unsafe { change_memory_protect(handle, mem_info.BaseAddress, mem_info.RegionSize, rwx).context("couldn't change memory protect flags to allow rwx")? };

    // try patching game memory
    try_patch(&pm_handle).context("couldn't apply patch to autoskip cutscenes")?;

    // change memory protection back to original value
    let _ = unsafe { change_memory_protect(handle, mem_info.BaseAddress, mem_info.RegionSize, oldflags).context("couldn't change memory protect flags back to old value after patching")? };

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum Msg {
    FolderSelect,
    LaunchGame { exit_after: bool },
}

fn main() {
    let mut installation = find_install_folder();

    let (sender, receiver) = fltk::app::channel();

    let mut window = fltk::window::Window::new(-1, -1, 480, 150, "");
    window.set_label(&format!("{} launcher v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")));

    fltk::dialog::message_title_default(&format!("{} launcher v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")));

    let mut flex = fltk::group::Flex::default().size_of_parent();
    flex.set_type(fltk::group::FlexType::Column);

    let mut flex_row = fltk::group::Flex::default().size_of_parent();
    flex_row.set_type(fltk::group::FlexType::Row);

    let mut folder_output = fltk::output::Output::default();
    folder_output.set_value(
        &installation
            .as_ref()
            .map(|i| std::borrow::Cow::Owned(format!("{}", i.dir.display())))
            .unwrap_or(std::borrow::Cow::Borrowed("select game install path")),
    );

    let mut folder_select_button = fltk::button::Button::default().with_label("Choose game folder");
    folder_select_button.emit(sender.clone(), Msg::FolderSelect);

    flex_row.end();
    flex_row.set_size(&folder_select_button, 150);
    flex.set_size(&flex_row, 32);

    let mut flex_row = fltk::group::Flex::default().size_of_parent();
    flex_row.set_type(fltk::group::FlexType::Row);

    let mut launch_game_button = fltk::button::Button::default().with_label("Launch Game");
    launch_game_button.emit(sender.clone(), Msg::LaunchGame { exit_after: false });

    let mut launch_game_and_exit_button = fltk::button::Button::default().with_label("Launch Game && Exit");
    launch_game_and_exit_button.emit(sender.clone(), Msg::LaunchGame { exit_after: true });
    flex_row.end();
    if installation.is_some() {
        launch_game_button.activate();
        launch_game_and_exit_button.activate();
    } else {
        launch_game_button.deactivate();
        launch_game_and_exit_button.deactivate();
    }

    flex.end();
    window.end();
    window.make_resizable(true);
    window.show();

    while fltk::app::wait() {
        let msg = receiver.recv();
        match msg {
            None => {
                // pass
            }
            Some(Msg::FolderSelect) => {
                let mut diag = fltk::dialog::FileDialog::new(fltk::dialog::FileDialogType::BrowseDir);
                diag.set_title("Choose game base installation path");
                diag.show();
                let dir = diag.filename();
                if !dir.as_os_str().is_empty() {
                    let exe = dir.join("bin/x64/ed8_3_PC.exe");
                    let ins = InstallFolder { dir, exe };
                    if install_folder_seems_valid(&ins) {
                        folder_output.set_label(&format!("{}", ins.dir.display()));
                        installation = Some(ins);
                        launch_game_button.activate();
                        launch_game_and_exit_button.activate();
                    } else {
                        fltk::dialog::alert_default(&format!("'{}' doesn't look a valid installation path", ins.dir.display()));
                    }
                }
            }
            Some(Msg::LaunchGame { exit_after }) => match (&installation).as_ref() {
                Some(InstallFolder { dir: _, exe }) => {
                    match launch_and_patch_game(&exe) {
                        Ok(()) => {}
                        Err(err) => {
                            fltk::dialog::alert_default(&format!("Error:\n{:?}", err));
                            continue;
                        }
                    }
                    if exit_after {
                        return;
                    }
                }
                None => {
                    unreachable!("button should be disable before valid game path is given");
                }
            },
        }
    }
}
