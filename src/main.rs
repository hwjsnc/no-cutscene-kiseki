#![windows_subsystem = "windows"]

mod patch;
mod win;

use patch::{find_install_path, identify_version, run_and_patch};

use clap::Parser;

use anyhow::Context as _;
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

fn main() -> anyhow::Result<()> {
    let Cmd { just_do_it } = Cmd::parse();
    if just_do_it {
        just_do_it_main()
    } else {
        gui_main();
        Ok(())
    }
}

fn just_do_it_main() -> anyhow::Result<()> {
    let path = find_install_path().ok_or_else(|| anyhow::Error::msg("couldn't find install path"))?;
    let version = identify_version(&path).context("couldn't determine game version")?;
    run_and_patch(&path, version).context("couldn't launch patched game")
}

fn gui_main() {
    let mut install_path = find_install_path();

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
        install_path
            .as_ref()
            .map(|dir| std::borrow::Cow::Owned(format!("{}", dir.display())))
            .unwrap_or(std::borrow::Cow::Borrowed("select game install path"))
            .as_ref(),
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
    if install_path.is_some() {
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
                    if dir.is_dir() {
                        folder_output.set_label(&format!("{}", dir.display()));
                        install_path = Some(dir);
                        launch_game_button.activate();
                        launch_game_and_exit_button.activate();
                    } else {
                        fltk::dialog::alert_default(&format!("Error: '{}' doesn't look a valid installation path", dir.display()));
                    }
                }
            }
            Some(Msg::LaunchGame { exit_after }) => match (&install_path).as_ref() {
                Some(path) => {
                    match identify_version(&path)
                        .context("couldn't identify game version")
                        .and_then(|version| run_and_patch(&path, version))
                        .context("couldn't launch game")
                    {
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
