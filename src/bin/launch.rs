use common::try_patch;

use anyhow::Context as _;
use blake2::Digest as _;
use fltk::prelude::GroupExt as _;
use fltk::prelude::InputExt as _;
use fltk::prelude::WidgetBase as _;
use fltk::prelude::WidgetExt as _;
use process_memory::TryIntoProcessHandle as _;

struct InstallFolder {
    dir: std::path::PathBuf,
    exe: std::path::PathBuf,
}

fn install_folder_seems_valid(installation: &InstallFolder) -> bool {
    installation.dir.is_dir()
        && std::fs::read(&installation.exe)
            .map(|data| {
                &blake2::Blake2b512::digest(&data)[..]
                    == hex_literal::hex!("bbce84df472cdeefd60464715aadfd7e35ca6a9107da320ea137b9424fc3009a4ecf55090f7a439a1ec4969df02bd190e46dd102c1f9bfbfa18d0b8babc989ff")
            })
            .unwrap_or(false)
}

fn find_install_folder() -> Option<InstallFolder> {
    let mut drvs = unsafe { windows::Win32::Storage::FileSystem::GetLogicalDrives() };
    if drvs == 0 {
        // 0 means Error
        drvs = 4; // 3rd-most significant bit is set, corresponds to drive C
    }
    let drives = ('A'..='Z').filter(|drv| drvs & (1 << (*drv as u32 - 'A' as u32)) != 0);
    let paths = [
        "GOG Games\\ToCS3",
        "Program Files (x86)\\Steam\\steamapps\\common\\The Legend of Heroes Trails of Cold Steel III",
        "SteamLibrary\\steamapps\\common\\The Legend of Heroes Trails of Cold Steel III",
    ];
    let absolute_folders = drives
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
            Some(Msg::LaunchGame { exit_after }) => {
                match (&installation).as_ref() {
                    Some(InstallFolder { dir: _, exe }) => {
                        let child = std::process::Command::new(exe).spawn().with_context(|| format!("couldn't launch {}", exe.display()));
                        // wait 10 ms for the game to start up
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        let patch_result = child
                            .and_then(|c| c.try_into_process_handle().map_err(anyhow::Error::msg))
                            .context("couldn't get game process handle")
                            .and_then(|handle| try_patch(&handle))
                            .context("couldn't apply patch to autoskip cutscenes");
                        match patch_result {
                            Ok(()) => {} // done; TODO: exit if checkbox is checked
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
                }
            }
        }
    }
}
