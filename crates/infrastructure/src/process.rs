use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Startet ein Programm, gibt ihm optional Text über stdin und wartet.
///
/// Geheime Werte gehen immer über stdin, nie über die Befehlszeile, denn die
/// kann jeder Prozess desselben Benutzers lesen.
pub fn run<I, S>(program: &Path, args: I, stdin: Option<&str>) -> std::io::Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // kein Konsolenfenster
    }
    let mut child = command.spawn()?;
    if let Some(text) = stdin {
        if let Some(mut pipe) = child.stdin.take() {
            pipe.write_all(text.as_bytes())?;
        }
    }
    child.wait_with_output()
}

/// Die letzten Zeilen einer Fehlerausgabe, gekürzt auf das Lesbare.
pub fn summary(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let tail = lines.iter().rev().take(3).rev().cloned().collect::<Vec<_>>().join(" ");
    tail.chars().take(300).collect()
}
