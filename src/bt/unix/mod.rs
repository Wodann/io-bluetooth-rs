use std::io;
use std::mem;
use std::os::{
    raw::c_int,
    unix::{
        io::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
        net::UnixStream,
    },
};
use std::ptr;

use crate::sys::c;

use crate::{BtAddr, BtProtocol};

pub fn discover_devices() -> io::Result<Vec<BtAddr>> {
    let device_id = unsafe { c::hci_get_route(ptr::null_mut()) };
    if device_id == -1 {
        return Err(io::Error::last_os_error());
    }

    let local_socket = unsafe { c::hci_open_dev(device_id) };
    if local_socket == -1 {
        return Err(io::Error::last_os_error());
    }

    let mut inquiry_infos = vec![c::inquiry_info::default(); 256];

    const TIMEOUT: c_int = 4; // 4 * 1.28 seconds
    let num_responses = unsafe {
        c::hci_inquiry(
            device_id,
            TIMEOUT,
            inquiry_infos.len() as c_int,
            ptr::null(),
            &mut inquiry_infos.as_mut_ptr(),
            c::IREQ_CACHE_FLUSH,
        )
    };
    if num_responses == -1 {
        return Err(io::Error::last_os_error());
    }

    inquiry_infos.truncate(num_responses as usize);
    let devices = inquiry_infos.iter().map(|ii| BtAddr(ii.bdaddr.b)).collect();

    if -1 == unsafe { c::hci_close_dev(local_socket) } {
        Err(io::Error::last_os_error())
    } else {
        Ok(devices)
    }
}

pub struct Socket(UnixStream);

pub struct BtSocket {
    inner: Socket,
}

impl BtSocket {
    pub fn new(protocol: BtProtocol) -> io::Result<Self> {
        let protocol = match protocol {
            BtProtocol::L2CAP => c::BTPROTO_L2CAP,
            BtProtocol::RFCOMM => c::BTPROTO_RFCOMM,
        };

        let fd = unsafe { c::socket(c::AF_BLUETOOTH, c::SOCK_STREAM, protocol) };
        if fd == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(BtSocket::from(fd))
        }
    }

    pub fn accept(&mut self) -> io::Result<BtSocket> {
        let fd = self.inner.0.as_raw_fd();
        let socket = unsafe { c::accept(fd, ptr::null_mut(), ptr::null_mut()) };
        if socket != -1 {
            Ok(BtSocket::from(socket))
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn bind(&mut self, address: BtAddr) -> io::Result<()> {
        let address = c::sockaddr_rc {
            rc_family: c::AF_BLUETOOTH as u16,
            rc_bdaddr: c::bdaddr_t { b: address.0 },
            rc_channel: 1,
        };
        let fd = self.inner.0.as_raw_fd();
        let res = unsafe {
            c::bind(
                fd,
                &address as *const c::sockaddr_rc as *const c::sockaddr,
                mem::size_of::<c::sockaddr_rc>() as u32,
            )
        };
        if res != -1 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn connect(&mut self, address: BtAddr) -> io::Result<()> {
        let address = c::sockaddr_rc {
            rc_family: c::AF_BLUETOOTH as u16,
            rc_bdaddr: c::bdaddr_t { b: address.0 },
            rc_channel: 1,
        };
        let fd = self.inner.0.as_raw_fd();
        let res = unsafe {
            c::connect(
                fd,
                &address as *const c::sockaddr_rc as *const c::sockaddr,
                mem::size_of::<c::sockaddr_rc>() as u32,
            )
        };
        if res != -1 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn peername(&self) -> io::Result<BtAddr> {
        let fd = self.inner.0.as_raw_fd();
        let mut address = c::sockaddr_rc::default();
        let mut len = mem::size_of::<c::sockaddr_rc>() as c::socklen_t;
        let res = unsafe {
            c::getpeername(
                fd,
                &mut address as *mut c::sockaddr_rc as *mut c::sockaddr,
                &mut len as *mut c::socklen_t,
            )
        };
        if res != -1 {
            Ok(BtAddr(address.rc_bdaddr.b))
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn socket(&self) -> &Socket {
        &self.inner
    }

    pub fn into_socket(self) -> Socket {
        self.inner
    }

    pub fn shutdown(self) -> io::Result<()> {
        let res = unsafe { c::shutdown(self.into_socket().0.into_raw_fd(), c::SHUT_RDWR) };
        if res != -1 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn sockname(&self) -> io::Result<BtAddr> {
        let fd = self.inner.0.as_raw_fd();
        let mut address = c::sockaddr_rc::default();
        let mut len = mem::size_of::<c::sockaddr_rc>() as c::socklen_t;
        let res = unsafe {
            c::getsockname(
                fd,
                &mut address as *mut c::sockaddr_rc as *mut c::sockaddr,
                &mut len as *mut c::socklen_t,
            )
        };
        if res != -1 {
            Ok(BtAddr(address.rc_bdaddr.b))
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

impl From<RawFd> for BtSocket {
    fn from(fd: RawFd) -> Self {
        Self {
            inner: Socket(unsafe { UnixStream::from_raw_fd(fd) }),
        }
    }
}

impl io::Read for BtSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.0.read(buf)
    }
}

impl io::Write for BtSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.0.flush()
    }
}
