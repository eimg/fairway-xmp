//! Import tool discovery settings without running user startup code in the PTY.
//! Exported values stay in memory and are never logged or persisted.
#[cfg(any(target_os = "macos", feature = "terminal"))]
use crate::config::{Layout, Result};
use std::ffi::OsString;

pub(crate) type Environment = Vec<(OsString, OsString)>;

#[cfg(target_os = "macos")]
pub(crate) fn load(layout: &Layout) -> Result<Environment> {
    use crate::command_output::output;
    let mut cmd = crate::process::command("/bin/zsh");
    // Interactive login startup loads brew/nvm/SDK/user PATH settings. Keep its
    // output out of the terminal, then start a clean shell with Fairway's own
    // prompt/history. Base64 framing preserves NULs and non-UTF8 Unix values.
    cmd.args(["-l", "-i", "-c", "unset HISTFILE; SAVEHIST=0; /usr/bin/printf '\\n__FAIRWAY_ENV_BEGIN__\\n'; /usr/bin/env -0 | /usr/bin/base64; /usr/bin/printf '\\n__FAIRWAY_ENV_END__\\n'"])
        .env("HOME", &layout.home)
        .env("TERM", "xterm-256color")
        .env("TERM_PROGRAM", "Fairway XMP")
        .current_dir(&layout.home);
    crate::node_tools::configure_shell(&mut cmd, layout);
    let text = output(cmd, 8, &|| false).map_err(|_| {
        "Could not load your shell environment. Check your Zsh startup files and try again."
            .to_string()
    })?;
    parse(&text)
}

#[cfg(target_os = "macos")]
fn parse(text: &str) -> Result<Environment> {
    use base64::Engine;
    use std::os::unix::ffi::OsStringExt;
    let invalid = || {
        "Could not read your shell's exported environment. Check your Zsh startup files and try again.".to_string()
    };
    let start = "__FAIRWAY_ENV_BEGIN__\n";
    let end = "\n__FAIRWAY_ENV_END__";
    let payload = text
        .rsplit_once(start)
        .and_then(|(_, rest)| rest.split_once(end).map(|(data, _)| data))
        .ok_or_else(invalid)?;
    let encoded: Vec<u8> = payload
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid())?;
    if !bytes.ends_with(&[0]) {
        return Err(invalid());
    }
    let env: Environment = bytes
        .split(|byte| *byte == 0)
        .filter_map(|entry| {
            let equal = entry.iter().position(|byte| *byte == b'=')?;
            if equal == 0 {
                return None;
            }
            Some((
                OsString::from_vec(entry[..equal].to_vec()),
                OsString::from_vec(entry[equal + 1..].to_vec()),
            ))
        })
        .collect();
    if !env.iter().any(|(key, _)| key == "PATH") {
        return Err(invalid());
    }
    Ok(env)
}

#[cfg(all(windows, feature = "terminal"))]
pub(crate) fn load(_: &Layout) -> Result<Environment> {
    Ok(windows_environment())
}

#[cfg(windows)]
pub(crate) fn windows_environment() -> Environment {
    use winreg::{enums::*, RegKey};
    let mut values: std::collections::BTreeMap<String, (OsString, OsString)> = std::env::vars_os()
        .map(|(key, value)| (key.to_string_lossy().to_lowercase(), (key, value)))
        .collect();
    let mut paths = Vec::new();
    let mut expand_keys = std::collections::HashSet::new();
    for (hive, location) in [
        (
            HKEY_LOCAL_MACHINE,
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
        ),
        (HKEY_CURRENT_USER, "Environment"),
    ] {
        if let Ok(key) = RegKey::predef(hive).open_subkey(location) {
            for (name, value) in key.enum_values().filter_map(|entry| entry.ok()) {
                if ![REG_SZ, REG_EXPAND_SZ].contains(&value.vtype) {
                    continue;
                }
                let value_type = value.vtype.clone();
                let words: Vec<u16> = value
                    .bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| u16::from_le_bytes(*p))
                    .take_while(|word| *word != 0)
                    .collect();
                let Ok(value) = String::from_utf16(&words) else {
                    continue;
                };
                if name.eq_ignore_ascii_case("PATH") {
                    paths.push((value, value_type == REG_EXPAND_SZ));
                } else {
                    if value_type == REG_EXPAND_SZ {
                        expand_keys.insert(name.to_lowercase());
                    } else {
                        expand_keys.remove(&name.to_lowercase());
                    }
                    values.insert(name.to_lowercase(), (name.into(), value.into()));
                }
            }
        }
    }
    // Resolve variable references against the fresh registry values as well as
    // the process environment (for APPDATA/USERPROFILE and installer settings).
    for _ in 0..8 {
        let expanded: Vec<_> = values
            .iter()
            .map(|(key, (name, value))| {
                (
                    key.clone(),
                    (
                        name.clone(),
                        if expand_keys.contains(key) {
                            expand_windows(&value.to_string_lossy(), &values).into()
                        } else {
                            value.clone()
                        },
                    ),
                )
            })
            .collect();
        let mut changed = false;
        for (key, next) in expanded {
            if values.get(&key) != Some(&next) {
                changed = true;
                values.insert(key, next);
            }
        }
        if !changed {
            break;
        }
    }
    if !paths.is_empty() {
        let path = paths
            .iter()
            .map(|(path, expand)| {
                if *expand {
                    expand_windows(path, &values)
                } else {
                    path.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(";");
        values.insert("path".into(), ("PATH".into(), path.into()));
    }
    values.into_values().collect()
}

#[cfg(windows)]
fn expand_windows(
    value: &str,
    variables: &std::collections::BTreeMap<String, (OsString, OsString)>,
) -> String {
    let mut result = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        result.push_str(&rest[..start]);
        let Some(end) = rest[start + 1..].find('%').map(|index| start + 1 + index) else {
            result.push_str(&rest[start..]);
            return result;
        };
        match variables.get(&rest[start + 1..end].to_lowercase()) {
            Some((_, replacement)) => {
                if result.len().saturating_add(replacement.len()) > 128 * 1024 {
                    return value.to_string();
                }
                result.push_str(&replacement.to_string_lossy());
            }
            None => result.push_str(&rest[start..=end]),
        }
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    result
}

/// Keep the controlled terminal's startup/history/prompt and working directory.
/// Tool variables (PATH, SDK homes, package-manager settings, etc.) are imported.
#[cfg(any(feature = "terminal", all(test, target_os = "macos")))]
pub(crate) fn is_tool_variable(key: &std::ffi::OsStr) -> bool {
    !matches!(
        key.to_string_lossy().to_ascii_uppercase().as_str(),
        "PATH"
            | "ZDOTDIR"
            | "ENV"
            | "BASH_ENV"
            | "HISTFILE"
            | "HISTSIZE"
            | "SAVEHIST"
            | "PROMPT"
            | "RPROMPT"
            | "PS1"
            | "PS2"
            | "PS4"
            | "TERM"
            | "COLORTERM"
            | "TERM_PROGRAM"
            | "PWD"
            | "OLDPWD"
            | "SHLVL"
            | "_"
    )
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use base64::Engine;
    #[test]
    fn shell_noise_and_arbitrary_exported_bytes_do_not_corrupt_environment() {
        use std::os::unix::ffi::OsStrExt;
        let bytes =
            b"PATH=/tmp/Student's tools:/usr/bin\0SDK_HOME=/tmp/hello\nworld=one\0RAW=\xff\0";
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let text = format!("\x1b]1337;hello\x07profile output\n__FAIRWAY_ENV_BEGIN__\n{encoded}\n__FAIRWAY_ENV_END__\nexit output");
        let env = parse(&text).unwrap();
        assert!(env
            .iter()
            .any(|(key, value)| key == "SDK_HOME" && value == "/tmp/hello\nworld=one"));
        assert!(env
            .iter()
            .any(|(key, value)| key == "RAW" && value.as_bytes() == [0xff]));
        assert!(parse("profile failed").is_err());
        assert!(
            parse("__FAIRWAY_ENV_BEGIN__\nPATH=/should-not-be-logged\n__FAIRWAY_ENV_END__")
                .unwrap_err()
                .contains("exported environment")
        );
        assert!(!is_tool_variable("ZDOTDIR".as_ref()));
        assert!(!is_tool_variable("HISTFILE".as_ref()));
        assert!(is_tool_variable("JAVA_HOME".as_ref()));
    }
}
