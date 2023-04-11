use common::{find_install_folder, install_folder_seems_valid, run_and_patch_game, InstallFolder};

use clap::Parser;

use fltk::prelude::GroupExt as _;
use fltk::prelude::InputExt as _;
use fltk::prelude::WidgetBase as _;
use fltk::prelude::WidgetExt as _;

#[derive(Parser)]
struct Cmd {
    /// Don't ask questions, just do your best. No GUI.
    #[arg(long)]
    just_do_it: bool,
}

#[derive(Debug, Clone, Copy)]
enum Msg {
    FolderSelect,
    LaunchGame { exit_after: bool },
}

fn main() {
    let Cmd { just_do_it } = Cmd::parse();
    if just_do_it {
        just_do_it_main()
    } else {
        gui_main()
    }
}

fn just_do_it_main() {
    let installation = find_install_folder().unwrap();
    run_and_patch_game(&installation.exe).unwrap();
}

fn gui_main() {
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
                    match run_and_patch_game(&exe) {
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
