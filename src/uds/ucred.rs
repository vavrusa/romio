use libc::{gid_t, uid_t};

/// Credentials of a process
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct UCred {
    /// UID (user ID) of the process
    pub uid: uid_t,
    /// GID (group ID) of the process
    pub gid: gid_t,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) use self::impl_linux::get_peer_cred;

#[cfg(any(
    target_os = "dragonfly",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
pub(crate) use self::impl_macos::get_peer_cred;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) mod impl_linux {
    use crate::uds::UnixStream;
    use libc::{c_void, getsockopt, socklen_t, SOL_SOCKET, SO_PEERCRED};
    use std::os::unix::io::AsRawFd;
    use std::{io, mem};

    use libc::ucred;

    pub(crate) fn get_peer_cred(sock: &UnixStream) -> io::Result<super::UCred> {
        unsafe {
            let raw_fd = sock.as_raw_fd();

            let mut ucred = ucred {
                pid: 0,
                uid: 0,
                gid: 0,
            };

            let ucred_size = mem::size_of::<ucred>();

            // These paranoid checks should be optimized-out
            assert!(mem::size_of::<u32>() <= mem::size_of::<usize>());
            assert!(ucred_size <= u32::max_value() as usize);

            let mut ucred_size = ucred_size as socklen_t;

            let ret = getsockopt(
                raw_fd,
                SOL_SOCKET,
                SO_PEERCRED,
                &mut ucred as *mut ucred as *mut c_void,
                &mut ucred_size,
            );
            if ret == 0 && ucred_size as usize == mem::size_of::<ucred>() {
                Ok(super::UCred {
                    uid: ucred.uid,
                    gid: ucred.gid,
                })
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[cfg(any(
    target_os = "dragonfly",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
pub(crate) mod impl_macos {
    use crate::uds::UnixStream;
    use libc::getpeereid;
    use std::os::unix::io::AsRawFd;
    use std::{io, mem};

    pub(crate) fn get_peer_cred(sock: &UnixStream) -> io::Result<super::UCred> {
        unsafe {
            let raw_fd = sock.as_raw_fd();

            let mut cred = mem::MaybeUninit::<super::UCred>::uninit();
            let ret = {
                let cred_mut = cred.as_mut_ptr();
                getpeereid(raw_fd, &mut (*cred_mut).uid, &mut (*cred_mut).gid)
            };

            if ret == 0 {
                Ok(cred.assume_init())
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

// Note that LOCAL_PEERCRED is not supported on DragonFly (yet). So do not run tests.
#[cfg(not(target_os = "dragonfly"))]
#[cfg(test)]
mod test {
    use crate::uds::UnixStream;
    use libc::getegid;
    use libc::geteuid;

    #[test]
    #[cfg_attr(
        target_os = "freebsd",
        ignore = "Requires FreeBSD 12.0 or later. https://bugs.freebsd.org/bugzilla/show_bug.cgi?id=176419"
    )]
    #[cfg_attr(
        target_os = "netbsd",
        ignore = "NetBSD does not support getpeereid() for sockets created by socketpair()"
    )]
    fn test_socket_pair() {
        let (a, b) = UnixStream::pair().unwrap();
        let cred_a = a.peer_cred().unwrap();
        let cred_b = b.peer_cred().unwrap();
        assert_eq!(cred_a, cred_b);

        let uid = unsafe { geteuid() };
        let gid = unsafe { getegid() };

        assert_eq!(cred_a.uid, uid);
        assert_eq!(cred_a.gid, gid);
    }
}
