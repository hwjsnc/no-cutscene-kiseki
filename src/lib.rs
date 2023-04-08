use anyhow::Context as _;
use process_memory::CopyAddress as _;
use process_memory::PutAddress as _;
use process_memory::TryIntoProcessHandle as _;
use sysinfo::PidExt as _;
use sysinfo::ProcessExt as _;
use sysinfo::SystemExt as _;

#[tracing::instrument(level = "debug", ret(Debug))]
fn find_pid_of(name: &str) -> anyhow::Result<sysinfo::Pid> {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(sysinfo::ProcessRefreshKind::new());
    let mut iter = system.processes_by_exact_name(name);
    let p = iter.next().with_context(|| format!("couldn't find process {}. is the game running?", name))?;
    anyhow::ensure!(iter.next().is_none(), anyhow::Error::msg("found more than one process, unsure which to patch"));
    anyhow::Ok(p.pid())
}

#[tracing::instrument(level = "debug", ret(Debug))]
pub fn get_process_handle(name: &str) -> anyhow::Result<process_memory::ProcessHandle> {
    let sysinfo_pid = find_pid_of(name)?;
    let process_memory_pid = process_memory::Pid::try_from(sysinfo_pid.as_u32()).context("couldn't convert sysinfo pid to process_memory pid")?;
    process_memory::Pid::try_into_process_handle(&process_memory_pid).context("couldn't get process handle")
}

#[tracing::instrument(level = "debug", skip(handle, expected, new))]
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
        tracing::event!(tracing::Level::DEBUG, "got expected values");
        if let Some(x) = new {
            // TODO: fails on linux, probably need to ptrace the process first
            handle.put_address(offset, x).with_context(|| format!("patching {description}"))?;
            tracing::event!(tracing::Level::DEBUG, "patch applied");
        }
        anyhow::Ok(())
    } else if Some(&actual) == new {
        Err(anyhow::Error::msg(format!("unexpected {description}: patch may have been applied already!")))
    } else {
        Err(anyhow::Error::msg(format!("unexpected {description}: expected {expected:?}, got {actual:?}")))
    }
}

#[tracing::instrument(level = "debug")]
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
