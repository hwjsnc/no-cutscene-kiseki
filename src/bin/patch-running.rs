use common::{alert, get_process_handle, message, try_patch};

use anyhow::Context as _;

fn main() -> std::process::ExitCode {
    match my_main() {
        Ok(_) => std::process::ExitCode::SUCCESS,
        Err(_) => std::process::ExitCode::FAILURE,
    }
}

fn my_main() -> Result<(), ()> {
    fltk::dialog::message_title_default(&format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")));

    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::ACTIVE)
            .with_env_filter(
                tracing_subscriber::filter::EnvFilter::builder()
                    .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                    .from_env_lossy(),
            )
            //.pretty()
            .compact()
            .finish(),
    )
    .unwrap();

    let handle = get_process_handle("ed8_3_PC.exe")
        .context("couldn't find ed8_3_PC.exe")
        .map_err(|err| alert(format!("Error:\n{:?}", err)))?;

    try_patch(&handle)
        .context("couldn't apply patch to autoskip cutscenes")
        .map_err(|err| alert(format!("Error:\n{:?}\n\n\nWhen in doubt, please restart the game!", err)))?;

    message("Success!\ncutscene autoskip patch has been applied.");
    Ok(())
}
