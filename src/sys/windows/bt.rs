use std::cmp;
use std::io;
use std::mem;
use std::net::{self, Shutdown};
use std::os::raw::{c_char, c_int, c_long, c_ulong};
use std::ptr;
use std::sync::{Once, ONCE_INIT};
use std::time::Duration;

use crate::sys::{self, c};
use crate::sys_common::bt;
use crate::sys_common::{AsInner, FromInner, IntoInner};

use crate::bt::{BtAddr, BtProtocol};

pub mod btc {
    pub use crate::sys::c::SOCKADDR as sockaddr;
    pub use crate::sys::c::SOCKADDR_STORAGE_LH as sockaddr_storage;
    pub use crate::sys::c::*;
    pub use std::os::raw::c_int as socklen_t;
    pub use std::os::raw::c_int as wrlen_t;
}

pub struct Socket(c::SOCKET);

fn init() {
    static START: Once = ONCE_INIT;

    START.call_once(|| {
        // Initialize winsock through the standard library by just creating a
        // dummy socket. Whether this is successful or not we drop the result as
        // libstd will be sure to have initialized winsock.
        let _ = net::UdpSocket::bind("127.0.0.1:34254");
    });
}

/// Returns the last error from the Windows socket interface.
fn last_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { c::WSAGetLastError() })
}

#[doc(hidden)]
pub trait IsMinusOne {
    fn is_minus_one(&self) -> bool;
}

macro_rules! impl_is_minus_one {
    ($($t:ident)*) => ($(impl IsMinusOne for $t {
        fn is_minus_one(&self) -> bool {
            *self == -1
        }
    })*)
}

impl_is_minus_one! { i8 i16 i32 i64 isize }

/// Checks if the signed integer is the Windows constant `SOCKET_ERROR` (-1)
/// and if so, returns the last error from the Windows socket interface. This
/// function must be called before another call to the socket API is made.
pub fn cvt<T: IsMinusOne>(t: T) -> io::Result<T> {
    if t.is_minus_one() {
        Err(last_error())
    } else {
        Ok(t)
    }
}

/// Just to provide the same interface as sys/unix/bt.rs
pub fn cvt_r<T, F>(mut f: F) -> io::Result<T>
where
    T: IsMinusOne,
    F: FnMut() -> T,
{
    cvt(f())
}

impl Socket {
    pub fn new(protocol: BtProtocol) -> io::Result<Self> {
        init();

        let protocol = match protocol {
            BtProtocol::L2CAP => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "L2CAP is currently not supported on Windows",
                ))
            } //c::BTHPROTO_L2CAP,
            BtProtocol::RFCOMM => c::BTHPROTO_RFCOMM,
        };
        let socket = unsafe {
            match c::WSASocketW(
                c::AF_BTH as c_int,
                c::SOCK_STREAM,
                protocol as c_int,
                ptr::null_mut(),
                0,
                c::WSA_FLAG_OVERLAPPED,
            ) {
                c::INVALID_SOCKET => Err(last_error()),
                n => Ok(Socket(n)),
            }
        }?;
        socket.set_no_inherit()?;
        Ok(socket)
    }

    pub fn accept(&self) -> io::Result<(Socket, BtAddr)> {
        let mut addr = c::SOCKADDR_BTH::default();
        let mut len = mem::size_of::<c::SOCKADDR_BTH>() as c_int;

        let socket = unsafe {
            match c::accept(self.0, &mut addr as *mut _ as *mut _, &mut len) {
                c::INVALID_SOCKET => Err(last_error()),
                n => Ok(Socket(n)),
            }
        }?;
        socket.set_no_inherit()?;

        Ok((
            socket,
            BtAddr::nap_sap(c::GET_NAP(addr.btAddr), c::GET_SAP(addr.btAddr)),
        ))
    }

    pub fn connect_timeout(&self, addr: BtAddr, timeout: Duration) -> io::Result<()> {
        self.set_nonblocking(true)?;
        let r = {
            let addr = c::SOCKADDR_BTH {
                addressFamily: c::AF_BTH,
                btAddr: addr.into(),
                // serviceClassId: protocol_guid(self.protocol),
                ..Default::default()
            };

            cvt(unsafe {
                c::connect(
                    self.0,
                    &addr as *const c::SOCKADDR_BTH as *const c::SOCKADDR,
                    mem::size_of::<c::SOCKADDR_BTH>() as i32,
                )
            })
        };
        self.set_nonblocking(false)?;

        match r {
            Ok(_) => return Ok(()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }

        if timeout.as_secs() == 0 && timeout.subsec_nanos() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot set a 0 duration timeout",
            ));
        }

        let timeout = {
            let tv_sec = timeout.as_secs() as c_long;
            let mut tv_usec = (timeout.subsec_nanos() / 1000) as c_long;
            if tv_sec == 0 && tv_usec == 0 {
                tv_usec = 1;
            }
            c::timeval { tv_sec, tv_usec }
        };

        let fds = {
            let mut fds = c::fd_set::default();
            fds.fd_count = 1;
            fds.fd_array[0] = self.0;
            fds
        };

        let mut writefds = fds;
        let mut errorfds = fds;

        let n =
            cvt(unsafe { c::select(1, ptr::null_mut(), &mut writefds, &mut errorfds, &timeout) })?;

        match n {
            0 => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "connection timed out",
            )),
            _ => {
                if writefds.fd_count != 1 {
                    if let Some(e) = self.take_error()? {
                        return Err(e);
                    }
                }
                Ok(())
            }
        }
    }

    pub fn duplicate(&self) -> io::Result<Socket> {
        let socket = {
            let mut info = c::WSAPROTOCOL_INFOW::default();
            cvt(unsafe { c::WSADuplicateSocketW(self.0, c::GetCurrentProcessId(), &mut info) })?;
            match unsafe {
                c::WSASocketW(
                    info.iAddressFamily,
                    info.iSocketType,
                    info.iProtocol,
                    &mut info,
                    0,
                    c::WSA_FLAG_OVERLAPPED,
                )
            } {
                c::INVALID_SOCKET => Err(last_error()),
                n => Ok(Socket(n)),
            }
        }?;
        socket.set_no_inherit()?;
        Ok(socket)
    }

    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, BtAddr)> {
        self.recv_from_with_flags(buf, c::MSG_PEEK)
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, BtAddr)> {
        self.recv_from_with_flags(buf, 0)
    }

    fn recv_from_with_flags(&self, buf: &mut [u8], flags: c_int) -> io::Result<(usize, BtAddr)> {
        let mut addr = c::SOCKADDR_BTH::default();
        let mut addrlen = mem::size_of::<c::SOCKADDR_BTH>() as c_int;
        let len = cmp::min(buf.len(), <c_int>::max_value() as usize) as c_int;

        match unsafe {
            c::recvfrom(
                self.0,
                buf.as_mut_ptr() as *mut c_char,
                len,
                flags,
                &mut addr as *mut _ as *mut _,
                &mut addrlen,
            )
        } {
            -1 if unsafe { c::WSAGetLastError() } == c::WSAESHUTDOWN => Ok((
                0,
                BtAddr::nap_sap(c::GET_NAP(addr.btAddr), c::GET_SAP(addr.btAddr)),
            )),
            -1 => Err(last_error()),
            n => Ok((
                n as usize,
                BtAddr::nap_sap(c::GET_NAP(addr.btAddr), c::GET_SAP(addr.btAddr)),
            )),
        }
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv_with_flags(buf, c::MSG_PEEK)
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv_with_flags(buf, 0)
    }

    fn recv_with_flags(&self, buf: &mut [u8], flags: c_int) -> io::Result<usize> {
        // On unix when a socket is shut down all further reads return 0, so we
        // do the same on windows to map a shut down to return EOF.
        let len = cmp::min(buf.len(), <c_int>::max_value() as usize) as c_int;
        match unsafe { c::recv(self.0, buf.as_mut_ptr() as *mut c_char, len, flags) } {
            -1 if unsafe { c::WSAGetLastError() } == c::WSAESHUTDOWN => Ok(0),
            -1 => Err(last_error()),
            n => Ok(n as usize),
        }
    }

    pub fn set_timeout(&self, dur: Option<Duration>, kind: c_int) -> io::Result<()> {
        let timeout = match dur {
            Some(dur) => {
                let timeout = sys::dur2timeout(dur);
                if timeout == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "cannot set a 0 duration timeout",
                    ));
                }
                timeout
            }
            None => 0,
        };
        bt::setsockopt(self, c::SOL_SOCKET, kind, timeout)
    }

    pub fn timeout(&self, kind: c_int) -> io::Result<Option<Duration>> {
        let raw: c_ulong = bt::getsockopt(self, c::SOL_SOCKET, kind)?;
        if raw == 0 {
            Ok(None)
        } else {
            let secs = raw / 1_000;
            let nsec = (raw % 1_000) * 1_000_000;
            Ok(Some(Duration::new(secs as u64, nsec as u32)))
        }
    }

    fn set_no_inherit(&self) -> io::Result<()> {
        sys::cvt(unsafe {
            c::SetHandleInformation(self.0 as c::HANDLE, c::HANDLE_FLAG_INHERIT, 0)
        })?;
        Ok(())
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        let mut nonblocking = nonblocking as c_ulong;
        cvt(unsafe { c::ioctlsocket(self.0, c::FIONBIO as c_int, &mut nonblocking) })?;
        Ok(())
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let how = match how {
            Shutdown::Write => c::SD_SEND,
            Shutdown::Read => c::SD_RECEIVE,
            Shutdown::Both => c::SD_BOTH,
        };
        cvt(unsafe { c::shutdown(self.0, how) })?;
        Ok(())
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        let raw: c_int = bt::getsockopt(self, c::SOL_SOCKET, c::SO_ERROR)?;
        if raw == 0 {
            Ok(None)
        } else {
            Ok(Some(io::Error::from_raw_os_error(raw as i32)))
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe { c::closesocket(self.0) };
    }
}

impl AsInner<c::SOCKET> for Socket {
    fn as_inner(&self) -> &c::SOCKET {
        &self.0
    }
}

impl FromInner<c::SOCKET> for Socket {
    fn from_inner(socket: c::SOCKET) -> Socket {
        Socket(socket)
    }
}

impl IntoInner<c::SOCKET> for Socket {
    fn into_inner(self) -> c::SOCKET {
        let ret = self.0;
        mem::forget(self);
        ret
    }
}

pub fn discover_devices() -> io::Result<Vec<BtAddr>> {
    init();

    let handle: c::HANDLE = {
        let mut query: c::WSAQUERYSETW = Default::default();
        query.dwSize = mem::size_of::<c::WSAQUERYSETW>() as u32;
        query.dwNameSpace = c::NS_BTH;

        let mut handle: c::HANDLE = std::ptr::null_mut();
        if 0 != unsafe {
            c::WSALookupServiceBeginW(
                &mut query,
                c::LUP_CONTAINERS | c::LUP_FLUSHCACHE,
                &mut handle,
            )
        } {
            Err(last_error())
        } else {
            Ok(handle)
        }
    }?;

    let mut addresses = Vec::new();
    let mut buffer: Vec<u8> = vec![0; mem::size_of::<c::WSAQUERYSETW>()];
    loop {
        let (query, mut len) = {
            let slice = &mut buffer[..];
            (
                slice.as_mut_ptr() as *mut c::WSAQUERYSETW,
                slice.len() as u32,
            )
        };

        unsafe {
            if 0 == c::WSALookupServiceNextW(
                handle,
                c::LUP_CONTAINERS | c::LUP_RETURN_ADDR,
                &mut len,
                query,
            ) {
                let query: c::WSAQUERYSETW = *query;
                let addr_info: c::CSADDR_INFO = *query.lpcsaBuffer;
                let addr = *(addr_info.RemoteAddr.lpSockaddr as *mut c::SOCKADDR_BTH);
                addresses.push(BtAddr::nap_sap(
                    c::GET_NAP(addr.btAddr),
                    c::GET_SAP(addr.btAddr),
                ));
            } else {
                let err = last_error();
                match err.raw_os_error().unwrap() as u32 {
                    c::WSA_E_NO_MORE => break,
                    c::WSAEFAULT => buffer.resize_with(len as usize, Default::default),
                    _ => return Err(err),
                }
            }
        };
    }

    if 0 != unsafe { c::WSALookupServiceEnd(handle) } {
        Err(last_error())
    } else {
        Ok(addresses)
    }
}

fn protocol_guid(protocol: BtProtocol) -> c::GUID {
    match protocol {
        BtProtocol::L2CAP => c::L2CAP_PROTOCOL_UUID,
        BtProtocol::RFCOMM => c::RFCOMM_PROTOCOL_UUID,
    }
}

impl Into<u64> for BtAddr {
    fn into(self) -> u64 {
        let sap = u32::from_le_bytes([self.0[0], self.0[1], self.0[2], self.0[3]]);
        let nap = u16::from_le_bytes([self.0[4], self.0[5]]);
        c::SET_NAP_SAP(nap, sap)
    }
}

impl<'a> Into<BtAddr> for &'a btc::sockaddr_storage {
    fn into(self) -> BtAddr {
        let sab: &'a c::SOCKADDR_BTH = unsafe { &*(self as *const _ as *const _) };
        BtAddr::nap_sap(c::GET_NAP(sab.btAddr), c::GET_SAP(sab.btAddr))
    }
}

impl Into<(btc::sockaddr_storage, btc::socklen_t)> for BtAddr {
    fn into(self) -> (btc::sockaddr_storage, btc::socklen_t) {
        let mut addr = btc::sockaddr_storage {
            ss_family: c::AF_BTH,
            ..Default::default()
        };

        let sab: &mut c::SOCKADDR_BTH = unsafe { &mut *(&mut addr as *mut _ as *mut _) };
        sab.btAddr = self.into();
        sab.serviceClassId = c::RFCOMM_PROTOCOL_UUID;

        (addr, mem::size_of::<c::SOCKADDR_BTH>() as c_int)
    }
}
