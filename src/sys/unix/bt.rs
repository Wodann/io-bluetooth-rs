use std::cmp;
use std::io;
use std::mem;
use std::net::Shutdown;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::time::{Duration, Instant};

mod libbt {
    pub use libbluetooth::bluetooth::{bdaddr_t, BTPROTO_L2CAP, BTPROTO_RFCOMM};
    pub use libbluetooth::hci::{inquiry_info, IREQ_CACHE_FLUSH};
    pub use libbluetooth::hci_lib::{hci_close_dev, hci_get_route, hci_inquiry, hci_open_dev};
    pub use libbluetooth::rfcomm::sockaddr_rc;
}

use libc;

use crate::bt::{BtAddr, BtProtocol};
use crate::sys::fd::FileDesc;
use crate::sys_common::bt::{getsockopt, setsockopt};
use crate::sys_common::{AsInner, FromInner, IntoInner};

pub use crate::sys::{cvt, cvt_r};

pub mod btc {
    pub use libc::size_t as wrlen_t;
    pub use libc::*;
}

// Another conditional constant for name resolution: MacOS and iOS use
// SO_NOSIGPIPE as a setsockopt flag to disable SIGPIPE emission on socket.
// Other platforms do otherwise.
#[cfg(not(target_os = "linux"))]
use libc::SO_NOSIGPIPE;
#[cfg(target_os = "linux")]
const SO_NOSIGPIPE: c_int = 0;

pub struct Socket(FileDesc);

impl Socket {
    pub fn new(protocol: BtProtocol) -> io::Result<Self> {
        let protocol = match protocol {
            BtProtocol::L2CAP => libbt::BTPROTO_L2CAP,
            BtProtocol::RFCOMM => libbt::BTPROTO_RFCOMM,
        };

        // On linux we first attempt to pass the SOCK_CLOEXEC flag to
        // atomically create the socket and set it as CLOEXEC. Support for
        // this option, however, was added in 2.6.27, and we still support
        // 2.6.18 as a kernel, so if the returned error is EINVAL we
        // fallthrough to the fallback.
        if cfg!(target_os = "linux") {
            let res = cvt(unsafe {
                libc::socket(
                    libc::AF_BLUETOOTH,
                    libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
                    protocol,
                )
            });
            match res {
                Ok(fd) => return Ok(Socket(FileDesc::new(fd))),
                Err(ref e) if e.raw_os_error() == Some(libc::EINVAL) => {}
                Err(e) => return Err(e),
            }
        }

        let fd = cvt(unsafe { libc::socket(libc::AF_BLUETOOTH, libc::SOCK_STREAM, protocol) })?;
        let fd = FileDesc::new(fd);
        fd.set_cloexec()?;
        let socket = Socket(fd);
        if cfg!(target_vendor = "apple") {
            setsockopt(&socket, libc::SOL_SOCKET, SO_NOSIGPIPE, 1)?;
        }
        Ok(socket)
    }

    pub fn accept(&self) -> io::Result<(Socket, BtAddr)> {
        let mut addr: libbt::sockaddr_rc = unsafe { mem::zeroed() };
        let mut len = mem::size_of::<libbt::sockaddr_rc>() as btc::socklen_t;

        // Unfortunately the only known way right now to accept a socket and
        // atomically set the CLOEXEC flag is to use the `accept4` syscall on
        // Linux. This was added in 2.6.28, however, and because we support
        // 2.6.18 we must detect this support dynamically.
        if cfg!(target_os = "linux") {
            let res = cvt_r(|| unsafe {
                libc::accept4(
                    self.0.raw(),
                    &mut addr as *mut _ as *mut _,
                    &mut len,
                    libc::SOCK_CLOEXEC,
                )
            });
            match res {
                Ok(fd) => return Ok((Socket(FileDesc::new(fd)), BtAddr(addr.rc_bdaddr.b))),
                Err(ref e) if e.raw_os_error() == Some(libc::ENOSYS) => {}
                Err(e) => return Err(e),
            }
        }

        let fd = cvt_r(|| unsafe {
            libc::accept(self.0.raw(), &mut addr as *mut _ as *mut _, &mut len)
        })?;
        let fd = FileDesc::new(fd);
        fd.set_cloexec()?;
        Ok((Socket(fd), BtAddr(addr.rc_bdaddr.b)))
    }

    pub fn connect_timeout(&self, addr: BtAddr, timeout: Duration) -> io::Result<()> {
        self.set_nonblocking(true)?;
        let r = {
            let addr = libbt::sockaddr_rc {
                rc_family: libc::AF_BLUETOOTH as u16,
                rc_bdaddr: libbt::bdaddr_t { b: addr.0 },
                rc_channel: 1,
            };
            cvt(unsafe {
                libc::connect(
                    self.0.raw(),
                    &addr as *const _ as *const _,
                    mem::size_of_val(&addr) as libc::socklen_t,
                )
            })
        };
        self.set_nonblocking(false)?;

        match r {
            Ok(_) => return Ok(()),
            // There's no ErrorKind for EINPROGRESS
            Err(ref e) if e.raw_os_error() == Some(libc::EINPROGRESS) => {}
            Err(e) => return Err(e),
        }

        let mut pollfd = libc::pollfd {
            fd: self.0.raw(),
            events: libc::POLLOUT,
            revents: 0,
        };

        if timeout.as_secs() == 0 && timeout.subsec_nanos() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot set a 0 duration timeout",
            ));
        }

        let start = Instant::now();

        loop {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "connection timed out",
                ));
            }

            let timeout = timeout - elapsed;
            let mut timeout = timeout
                .as_secs()
                .saturating_mul(1_000)
                .saturating_add(timeout.subsec_nanos() as u64 / 1_000_000);
            if timeout == 0 {
                timeout = 1;
            }

            let timeout = cmp::min(timeout, c_int::max_value() as u64) as c_int;

            match unsafe { libc::poll(&mut pollfd, 1, timeout) } {
                -1 => {
                    let err = io::Error::last_os_error();
                    if err.kind() != io::ErrorKind::Interrupted {
                        return Err(err);
                    }
                }
                0 => {}
                _ => {
                    // linux returns POLLOUT|POLLERR|POLLHUP for refused connections (!), so look
                    // for POLLHUP rather than read readiness
                    if pollfd.revents & libc::POLLHUP != 0 {
                        let e = self.take_error()?.unwrap_or_else(|| {
                            io::Error::new(io::ErrorKind::Other, "no error set after POLLHUP")
                        });
                        return Err(e);
                    }

                    return Ok(());
                }
            }
        }
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv_with_flags(buf, libc::MSG_PEEK)
    }

    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, BtAddr)> {
        self.recv_from_with_flags(buf, libc::MSG_PEEK)
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv_with_flags(buf, 0)
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, BtAddr)> {
        self.recv_from_with_flags(buf, 0)
    }

    fn recv_from_with_flags(&self, buf: &mut [u8], flags: c_int) -> io::Result<(usize, BtAddr)> {
        let mut addr: libbt::sockaddr_rc = unsafe { mem::zeroed() };
        let mut addrlen = mem::size_of_val(&addr) as libc::socklen_t;

        let n = cvt(unsafe {
            libc::recvfrom(
                self.0.raw(),
                buf.as_mut_ptr() as *mut c_void,
                buf.len(),
                flags,
                &mut addr as *mut _ as *mut _,
                &mut addrlen,
            )
        })?;
        Ok((n as usize, BtAddr(addr.rc_bdaddr.b)))
    }

    fn recv_with_flags(&self, buf: &mut [u8], flags: c_int) -> io::Result<usize> {
        let ret = cvt(unsafe {
            libc::recv(
                self.0.raw(),
                buf.as_mut_ptr() as *mut c_void,
                buf.len(),
                flags,
            )
        })?;
        Ok(ret as usize)
    }

    pub fn set_timeout(&self, dur: Option<Duration>, kind: c_int) -> io::Result<()> {
        let timeout = match dur {
            Some(dur) => {
                if dur.as_secs() == 0 && dur.subsec_nanos() == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "cannot set a 0 duration timeout",
                    ));
                }

                let secs = if dur.as_secs() > libc::time_t::max_value() as u64 {
                    libc::time_t::max_value()
                } else {
                    dur.as_secs() as libc::time_t
                };
                let mut timeout = libc::timeval {
                    tv_sec: secs,
                    tv_usec: (dur.subsec_nanos() / 1_000) as libc::suseconds_t,
                };
                if timeout.tv_sec == 0 && timeout.tv_usec == 0 {
                    timeout.tv_usec = 1;
                }
                timeout
            }
            None => libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
        };
        setsockopt(self, libc::SOL_SOCKET, kind, timeout)
    }

    pub fn timeout(&self, kind: c_int) -> io::Result<Option<Duration>> {
        let raw: libc::timeval = getsockopt(self, libc::SOL_SOCKET, kind)?;
        if raw.tv_sec == 0 && raw.tv_usec == 0 {
            Ok(None)
        } else {
            let sec = raw.tv_sec as u64;
            let nsec = (raw.tv_usec as u32) * 1_000;
            Ok(Some(Duration::new(sec, nsec)))
        }
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let how = match how {
            Shutdown::Write => libc::SHUT_WR,
            Shutdown::Read => libc::SHUT_RD,
            Shutdown::Both => libc::SHUT_RDWR,
        };
        cvt(unsafe { libc::shutdown(self.0.raw(), how) })?;
        Ok(())
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        let mut nonblocking = nonblocking as c_int;
        cvt(unsafe { libc::ioctl(*self.as_inner(), libc::FIONBIO, &mut nonblocking) }).map(|_| ())
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        let raw: c_int = getsockopt(self, libc::SOL_SOCKET, libc::SO_ERROR)?;
        if raw == 0 {
            Ok(None)
        } else {
            Ok(Some(io::Error::from_raw_os_error(raw as i32)))
        }
    }

    pub fn duplicate(&self) -> io::Result<Socket> {
        self.0.duplicate().map(Socket)
    }
}

impl AsInner<c_int> for Socket {
    fn as_inner(&self) -> &c_int {
        self.0.as_inner()
    }
}

impl FromInner<c_int> for Socket {
    fn from_inner(fd: c_int) -> Socket {
        Socket(FileDesc::new(fd))
    }
}

impl IntoInner<c_int> for Socket {
    fn into_inner(self) -> c_int {
        self.0.into_raw()
    }
}

pub fn discover_devices() -> io::Result<Vec<BtAddr>> {
    let device_id = unsafe { libbt::hci_get_route(ptr::null_mut()) };
    if device_id == -1 {
        return Err(io::Error::last_os_error());
    }

    let local_socket = unsafe { libbt::hci_open_dev(device_id) };
    if local_socket == -1 {
        return Err(io::Error::last_os_error());
    }

    let mut inquiry_infos = vec![libbt::inquiry_info::default(); 256];

    const TIMEOUT: c_int = 4; // 4 * 1.28 seconds
    let num_responses = unsafe {
        libbt::hci_inquiry(
            device_id,
            TIMEOUT,
            inquiry_infos.len() as c_int,
            ptr::null(),
            &mut inquiry_infos.as_mut_ptr(),
            libbt::IREQ_CACHE_FLUSH,
        )
    };
    if num_responses == -1 {
        return Err(io::Error::last_os_error());
    }

    inquiry_infos.truncate(num_responses as usize);
    let devices = inquiry_infos.iter().map(|ii| BtAddr(ii.bdaddr.b)).collect();

    if -1 == unsafe { libbt::hci_close_dev(local_socket) } {
        Err(io::Error::last_os_error())
    } else {
        Ok(devices)
    }
}

impl<'a> Into<BtAddr> for &'a btc::sockaddr_storage {
    fn into(self) -> BtAddr {
        let addr: &'a libbt::sockaddr_rc = unsafe { &*(self as *const _ as *const _) };
        BtAddr(addr.rc_bdaddr.b)
    }
}

impl<'a> Into<(btc::sockaddr_storage, btc::socklen_t)> for &'a BtAddr {
    fn into(self) -> (btc::sockaddr_storage, btc::socklen_t) {
        let mut addr: btc::sockaddr_storage = unsafe { mem::zeroed() };

        let sarc: &mut libbt::sockaddr_rc = unsafe { &mut *(&mut addr as *mut _ as *mut _) };
        sarc.rc_family = libc::AF_BLUETOOTH as u16;
        sarc.rc_bdaddr.b = self.0;
        sarc.rc_channel = 1;

        (addr, mem::size_of::<libbt::sockaddr_rc>() as btc::socklen_t)
    }
}
