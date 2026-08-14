//! MCP 对端认证：SO_PEERCRED uid 校验（防御 socket 目录被其他进程访问）。

pub(crate) const MAX_CONSECUTIVE_REJECTIONS_BEFORE_LOG: u32 = 8;

/// Returns `true` when the peer uid on the accepted stream is allowed:
/// the server's own uid (the app) or root (uid 0), mirroring termux's
/// `LocalServerSocket` rule (`peerUid != appUid && peerUid != 0` → reject).
pub(crate) fn peer_uid_allowed(peer_uid: u32, own_uid: u32) -> bool {
    peer_uid == own_uid || peer_uid == 0
}

/// Reads SO_PEERCRED from a Unix stream fd. Returns `None` when
/// `getsockopt` fails (e.g. non-Linux platform, or fd not a unix socket).
#[cfg(any(target_os = "android", target_os = "linux"))]
pub(crate) fn peer_uid_of(fd: std::os::fd::RawFd) -> Option<u32> {
    // SAFETY: `ucred` is a plain POD struct; getsockopt writes into it when
    // it succeeds. The fd comes from a tokio UnixStream that outlives this
    // call, so the fd is valid for the duration.
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut libc::ucred as *mut libc::c_void,
            &mut len,
        )
    };
    if rc != 0 {
        log::debug!(
            "MCP server: getsockopt(SO_PEERCRED) failed: {}",
            std::io::Error::last_os_error()
        );
        return None;
    }
    Some(cred.uid)
}
