//! A GUI/Task-Scheduler process has no usable inherited terminal handles.
//! CREATE_NEW_CONSOLE via std::Command can still inherit redirected handles;
//! create the child without STARTF_USESTDHANDLES or handle inheritance instead.
use opentoken_core::cli_profiles::CliProvider;
use std::{ffi::OsStr, path::Path};
const CLEARED: &[&str] = &[
    "CODEX_API_KEY",
    "CODEX_ACCESS_TOKEN",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "CODEX_SQLITE_HOME",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SIMPLE",
    "NODE_OPTIONS",
    "GROK_API_KEY",
    "GROK_OAUTH_TOKEN",
    "XAI_API_KEY",
    "GROK_BASE_URL",
    "GROK_API_BASE",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "GH_HOST",
    "GH_PROMPT_DISABLED",
    "GIT_TERMINAL_PROMPT",
    "CI",
];
#[cfg(windows)]
pub struct LoginProcess(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for LoginProcess {}
#[cfg(not(windows))]
pub struct LoginProcess(std::process::Child);
#[cfg(windows)]
fn wide(value: &OsStr) -> anyhow::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    let mut result: Vec<_> = value.encode_wide().collect();
    if result.contains(&0) {
        anyhow::bail!("Invalid sign-in launch data.");
    }
    result.push(0);
    Ok(result)
}
#[cfg(windows)]
impl LoginProcess {
    pub fn spawn(executable: &Path, provider: CliProvider, home: &Path) -> anyhow::Result<Self> {
        Self::spawn_args(
            executable,
            provider.arguments(),
            provider.environment(),
            home,
        )
    }
    pub fn spawn_args(
        executable: &Path,
        args: &[&str],
        variable: &str,
        home: &Path,
    ) -> anyhow::Result<Self> {
        use windows::{
            core::{PCWSTR, PWSTR},
            Win32::{Foundation::CloseHandle, System::Threading::*},
        };
        if !executable.is_absolute() || !home.is_absolute() {
            anyhow::bail!("Sign-in requires absolute local paths.");
        }
        let app = wide(executable.as_os_str())?;
        let cwd = wide(home.as_os_str())?;
        // Only trusted static argument strings; no shell or frontend command text.
        let line = format!("\"{}\" {}", executable.to_string_lossy(), args.join(" "));
        let mut command = wide(OsStr::new(&line))?;
        let mut environment: Vec<(std::ffi::OsString, std::ffi::OsString)> = std::env::vars_os()
            .filter(|(key, _)| {
                !CLEARED
                    .iter()
                    .any(|name| key.to_string_lossy().eq_ignore_ascii_case(name))
                    && !key.to_string_lossy().eq_ignore_ascii_case(variable)
            })
            .collect();
        environment.push((variable.into(), home.as_os_str().to_owned()));
        environment.sort_by_key(|(key, _)| key.to_string_lossy().to_uppercase());
        let mut block = Vec::new();
        for (key, value) in environment {
            let mut pair = key;
            pair.push("=");
            pair.push(value);
            block.extend(wide(&pair)?);
        }
        block.push(0);
        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut info = PROCESS_INFORMATION::default();
        unsafe {
            CreateProcessW(
                PCWSTR(app.as_ptr()),
                Some(PWSTR(command.as_mut_ptr())),
                None,
                None,
                false,
                CREATE_NEW_CONSOLE | CREATE_UNICODE_ENVIRONMENT,
                Some(block.as_ptr().cast()),
                PCWSTR(cwd.as_ptr()),
                &startup,
                &mut info,
            )?;
            let _ = CloseHandle(info.hThread);
        }
        Ok(Self(info.hProcess))
    }
    pub fn try_wait(&mut self) -> anyhow::Result<Option<bool>> {
        use windows::Win32::System::Threading::*;
        let mut code = 0;
        unsafe {
            GetExitCodeProcess(self.0, &mut code)?;
        }
        Ok((code != 259).then_some(code == 0))
    }
    pub fn stop(&mut self) {
        use windows::Win32::System::Threading::*;
        unsafe {
            if self.try_wait().ok().flatten().is_none() {
                let _ = TerminateProcess(self.0, 1);
                let _ = WaitForSingleObject(self.0, 1000);
            }
        }
    }
}
#[cfg(windows)]
impl Drop for LoginProcess {
    fn drop(&mut self) {
        self.stop();
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(not(windows))]
impl LoginProcess {
    pub fn spawn(executable: &Path, provider: CliProvider, home: &Path) -> anyhow::Result<Self> {
        Self::spawn_args(
            executable,
            provider.arguments(),
            provider.environment(),
            home,
        )
    }
    pub fn spawn_args(
        executable: &Path,
        args: &[&str],
        variable: &str,
        home: &Path,
    ) -> anyhow::Result<Self> {
        let mut command = std::process::Command::new(executable);
        command.args(args).env(variable, home).current_dir(home);
        for key in CLEARED {
            command.env_remove(key);
        }
        Ok(Self(command.spawn()?))
    }
    pub fn try_wait(&mut self) -> anyhow::Result<Option<bool>> {
        Ok(self.0.try_wait()?.map(|s| s.success()))
    }
    pub fn stop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[cfg(not(windows))]
impl Drop for LoginProcess {
    fn drop(&mut self) {
        self.stop();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_nested_session_and_credential_overrides() {
        for name in [
            "CLAUDECODE",
            "CLAUDE_CODE_SIMPLE",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
        ] {
            assert!(CLEARED.contains(&name));
        }
    }
    #[cfg(windows)]
    #[test]
    fn embedded_nuls_are_rejected() {
        assert!(wide(OsStr::new("safe")).is_ok());
        assert!(wide(OsStr::new("bad\0value")).is_err());
    }
}
