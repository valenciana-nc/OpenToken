//! Narrow current-user Antigravity process/port discovery using documented Windows APIs.
//! No script process, process-memory read/injection, vault enumeration or browser access.
use super::{
    antigravity::{Desktop as DesktopHost, Endpoint},
    *,
};
use ::windows::{
    core::{w, BSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE},
        NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
        },
        Networking::WinSock::AF_INET,
        Security::{EqualSid, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER},
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoSetProxyBlanket, CoUninitialize,
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
            },
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            RemoteDesktop::ProcessIdToSessionId,
            Threading::{
                GetCurrentProcess, GetCurrentProcessId, GetProcessTimes, OpenProcess,
                OpenProcessToken, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
            Variant::{VARIANT, VT_BSTR},
            Wmi::{
                IWbemLocator, WbemLocator, WBEM_FLAG_CONNECT_USE_MAX_WAIT, WBEM_FLAG_FORWARD_ONLY,
                WBEM_FLAG_RETURN_IMMEDIATELY,
            },
        },
    },
};
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
pub(super) fn antigravity_credential() -> Result<Zeroizing<Vec<u8>>, Fault> {
    read_antigravity_credential(false)
}
pub(super) fn antigravity_cli_namespace_present() -> Result<bool, Fault> {
    match read_antigravity_credential(true) {
        Ok(_) => Ok(true),
        Err(f) if f.kind == FaultKind::NotSignedIn => Ok(false),
        Err(f) => Err(f),
    }
}
fn read_antigravity_credential(cli_namespace: bool) -> Result<Zeroizing<Vec<u8>>, Fault> {
    use ::windows::Win32::Security::Credentials::{
        CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
    };
    unsafe {
        let mut raw: *mut CREDENTIALW = std::ptr::null_mut();
        if let Err(e) = CredReadW(
            if cli_namespace {
                w!("gemini:antigravity-cli")
            } else {
                w!("gemini:antigravity")
            },
            CRED_TYPE_GENERIC,
            None,
            &mut raw,
        ) {
            return Err(if e.code().0 == 0x80070490u32 as i32 {
                Fault::new(FaultKind::NotSignedIn,"No Antigravity CLI sign-in was found in its vendor-owned Windows credential entry, and no supported desktop service is running. Open your signed-in Antigravity app/IDE or the official agy CLI; no login was started.")
            } else {
                Fault::new(FaultKind::Unreadable,"Windows could not read Antigravity CLI's exact credential entry. No other vault entry or browser data was read.")
            });
        }
        struct Owned(*mut CREDENTIALW);
        impl Drop for Owned {
            fn drop(&mut self) {
                unsafe {
                    CredFree(self.0.cast());
                }
            }
        }
        let owned = Owned(raw);
        if owned.0.is_null() {
            return Err(invalid());
        }
        let value = &*owned.0;
        let len = value.CredentialBlobSize as usize;
        if len == 0 || len > 65536 || value.CredentialBlob.is_null() {
            return Err(invalid());
        }
        Ok(Zeroizing::new(
            std::slice::from_raw_parts(value.CredentialBlob, len).to_vec(),
        ))
    }
}
fn key(path: &Path) -> String {
    crate::storage::path_key(path).replace('\\', "/")
}
pub(super) struct Desktop {
    roots: Vec<PathBuf>,
    roaming: PathBuf,
    home: PathBuf,
}
impl Desktop {
    pub fn new(storage: &Storage) -> Self {
        let mut bases = vec![storage.user_home.join("AppData/Local/Programs")];
        // Isolated test homes must never acquire an unrelated installed user's application.
        if storage.user_home == Storage::system().user_home {
            if let Some(p) = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
            {
                bases.push(p.join("Programs"));
            }
            if let Some(p) = std::env::var_os("ProgramFiles")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
            {
                bases.push(p);
            }
        }
        let roots = bases
            .into_iter()
            .flat_map(|p| [p.join("Antigravity"), p.join("Antigravity IDE")])
            .collect();
        let roaming = super::roaming(storage);
        Self {
            roots,
            roaming,
            home: storage.user_home.clone(),
        }
    }
}
fn win<T>(value: ::windows::core::Result<T>) -> Result<T, Fault> {
    value.map_err(|_|Fault::new(FaultKind::Unreadable,"Windows could not verify the existing Antigravity process. No local service was queried."))
}
fn token_user(process: HANDLE) -> Result<Vec<u64>, Fault> {
    unsafe {
        let mut token = HANDLE::default();
        win(OpenProcessToken(process, TOKEN_QUERY, &mut token))?;
        let token = Handle(token);
        let mut size = 0;
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut size);
        if size == 0 || size > 64 * 1024 {
            return Err(invalid());
        }
        let mut out = vec![0u64; (size as usize).div_ceil(8)];
        win(GetTokenInformation(
            token.0,
            TokenUser,
            Some(out.as_mut_ptr().cast()),
            size,
            &mut size,
        ))?;
        Ok(out)
    }
}
fn owner(process: HANDLE, pid: u32) -> Result<u64, Fault> {
    unsafe {
        let a = token_user(GetCurrentProcess())?;
        let b = token_user(process)?;
        win(EqualSid(
            (*(a.as_ptr() as *const TOKEN_USER)).User.Sid,
            (*(b.as_ptr() as *const TOKEN_USER)).User.Sid,
        ))?;
        let mut own = 0;
        let mut other = 0;
        win(ProcessIdToSessionId(GetCurrentProcessId(), &mut own))?;
        win(ProcessIdToSessionId(pid, &mut other))?;
        if own != other {
            return Err(invalid());
        }
        let (mut born, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        win(GetProcessTimes(
            process,
            &mut born,
            &mut exited,
            &mut kernel,
            &mut user,
        ))?;
        Ok(((born.dwHighDateTime as u64) << 32) | born.dwLowDateTime as u64)
    }
}
fn image(process: HANDLE) -> Result<PathBuf, Fault> {
    unsafe {
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        win(QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        ))?;
        Ok(PathBuf::from(
            String::from_utf16(&buf[..len as usize]).map_err(|_| invalid())?,
        ))
    }
}
fn ports(pid: u32) -> Result<Vec<u16>, Fault> {
    unsafe {
        let mut size = 0;
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        );
        if !(4..=4 * 1024 * 1024).contains(&size) {
            return Err(invalid());
        }
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        if GetExtendedTcpTable(
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        ) != 0
        {
            return Err(Fault::offline());
        }
        let table = &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let count = table.dwNumEntries as usize;
        if count > 65536 || 4 + count * std::mem::size_of_val(&table.table[0]) > buffer.len() * 8 {
            return Err(invalid());
        }
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), count);
        let mut result: Vec<_> = rows
            .iter()
            .filter(|r| {
                r.dwOwningPid == pid
                    && (r.dwLocalAddr == 0 || r.dwLocalAddr == u32::from_ne_bytes([127, 0, 0, 1]))
            })
            .map(|r| u16::from_be(r.dwLocalPort as u16))
            .filter(|p| *p != 0)
            .take(8)
            .collect();
        result.sort_unstable();
        result.dedup();
        Ok(result)
    }
}
fn command_line(pid: u32) -> Result<Zeroizing<String>, Fault> {
    unsafe {
        win(CoInitializeEx(None, COINIT_MULTITHREADED).ok())?;
        let _com = Com;
        let locator: IWbemLocator =
            win(CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER))?;
        let empty = BSTR::new();
        let service = win(locator.ConnectServer(
            &BSTR::from("ROOT\\CIMV2"),
            &empty,
            &empty,
            &empty,
            WBEM_FLAG_CONNECT_USE_MAX_WAIT.0,
            &empty,
            None,
        ))?;
        win(CoSetProxyBlanket(
            &service,
            10,
            0,
            None,
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        ))?;
        // PID has already passed executable, same-user and session checks. Never query unrelated command lines.
        let query = win(service.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from(format!(
                "SELECT CommandLine FROM Win32_Process WHERE ProcessId = {pid}"
            )),
            WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
            None,
        ))?;
        let mut objects = [None];
        let mut count = 0;
        win(query.Next(1500, &mut objects, &mut count).ok())?;
        if count != 1 {
            return Err(invalid());
        }
        let object = objects[0].take().ok_or_else(invalid)?;
        let mut value = VARIANT::default();
        win(object.Get(w!("CommandLine"), 0, &mut value, None, None))?;
        let inner = &value.Anonymous.Anonymous;
        if inner.vt != VT_BSTR {
            return Err(invalid());
        }
        let line = &*inner.Anonymous.bstrVal;
        if line.len() > 32768 {
            return Err(invalid());
        }
        Ok(Zeroizing::new(line.to_string()))
    }
}
// CommandLineToArgvW quoting rules for flags, without allocating an unmanaged secret buffer.
pub(super) fn arguments(line: &str) -> Result<Vec<Zeroizing<String>>, Fault> {
    let mut out = vec![];
    let mut arg = Zeroizing::new(String::new());
    let mut quoted = false;
    let mut active = false;
    let mut it = line.chars().peekable();
    while let Some(ch) = it.next() {
        if ch == '\\' {
            let mut n = 1;
            while it.peek() == Some(&'\\') {
                it.next();
                n += 1;
            }
            if it.peek() == Some(&'"') {
                for _ in 0..n / 2 {
                    arg.push('\\');
                }
                it.next();
                if n % 2 == 1 {
                    arg.push('"');
                } else {
                    quoted = !quoted;
                }
            } else {
                for _ in 0..n {
                    arg.push('\\');
                }
            }
            active = true;
        } else if ch == '"' {
            quoted = !quoted;
            active = true;
        } else if ch.is_whitespace() && !quoted {
            if active {
                out.push(arg);
                arg = Zeroizing::new(String::new());
                active = false;
            }
        } else {
            arg.push(ch);
            active = true;
        }
        if out.len() > 128 {
            return Err(invalid());
        }
    }
    if quoted {
        return Err(invalid());
    }
    if active {
        out.push(arg);
    }
    Ok(out)
}
fn flag(args: &[Zeroizing<String>], name: &str) -> Result<Option<Zeroizing<String>>, Fault> {
    let mut result = None;
    for (i, a) in args.iter().enumerate() {
        let value = if a.as_str() == name {
            Some(args.get(i + 1).ok_or_else(invalid)?.to_string())
        } else {
            a.strip_prefix(&format!("{name}=")).map(str::to_owned)
        };
        if let Some(value) = value {
            if result.is_some() {
                return Err(invalid());
            }
            result = Some(Zeroizing::new(value));
        }
    }
    Ok(result)
}
impl DesktopHost for Desktop {
    fn locate(&self, profile: &Path) -> Result<Vec<Endpoint>, Fault> {
        unsafe {
            let expected = [
                self.roaming.join("Antigravity"),
                self.roaming.join("Antigravity IDE"),
            ];
            if !expected.iter().any(|p| key(p) == key(profile)) {
                return Err(invalid());
            }
            let snapshot = Handle(win(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0))?);
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut next = Process32FirstW(snapshot.0, &mut entry).is_ok();
            let mut result = vec![];
            let mut candidates = 0;
            while next {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase();
                if [
                    "language_server_windows_x64.exe",
                    "language_server_windows_arm64.exe",
                    "language_server.exe",
                ]
                .contains(&name.as_str())
                {
                    candidates += 1;
                    if candidates > 16 {
                        break;
                    }
                    if let Ok(handle) = OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION,
                        false,
                        entry.th32ProcessID,
                    ) {
                        let process = Handle(handle);
                        if let (Ok(path), Ok(born)) =
                            (image(process.0), owner(process.0, entry.th32ProcessID))
                        {
                            let image_key = key(&path);
                            let trusted = self
                                .roots
                                .iter()
                                .any(|root| image_key.starts_with(&(key(root) + "/")));
                            if trusted && crate::monitoring::safe_file(&path) {
                                let line = command_line(entry.th32ProcessID)?;
                                let args = arguments(&line)?;
                                let data = flag(&args, "--app_data_dir")?;
                                let ide = flag(&args, "--override_ide_name")?
                                    .or(flag(&args, "--ide_name")?);
                                let tag = if profile
                                    .file_name()
                                    .is_some_and(|s| s == "Antigravity IDE")
                                {
                                    "antigravity-ide"
                                } else {
                                    "antigravity"
                                };
                                let matched = data.as_deref().is_some_and(|s| {
                                    s.eq_ignore_ascii_case(tag)
                                        || key(Path::new(s)) == key(profile)
                                        || key(Path::new(s))
                                            == key(&self.home.join(".gemini").join(tag))
                                });
                                if matched
                                    && ide.as_deref().is_none_or(|s| {
                                        s.eq_ignore_ascii_case(tag) || s == "antigravity"
                                    })
                                {
                                    if let Some(csrf) = flag(&args, "--csrf_token")?.filter(|s| {
                                        !s.is_empty()
                                            && s.len() <= 1024
                                            && s.bytes().all(|b| b.is_ascii_graphic())
                                    }) {
                                        // Recheck this handle's lifetime and image after WMI, before binding listeners.
                                        if owner(process.0, entry.th32ProcessID)? != born
                                            || key(&image(process.0)?) != image_key
                                        {
                                            return Err(Fault::changed());
                                        }
                                        for port in ports(entry.th32ProcessID)? {
                                            result.push(Endpoint {
                                                pid: entry.th32ProcessID,
                                                born,
                                                port,
                                                csrf: Zeroizing::new(csrf.to_string()),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if result.len() > 8 {
                    return Err(denied("Too many Antigravity listeners were found. Select the intended running app before connecting."));
                }
                next = Process32NextW(snapshot.0, &mut entry).is_ok();
            }
            Ok(result)
        }
    }
}
