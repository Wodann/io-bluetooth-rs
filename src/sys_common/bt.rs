use std::cmp;
use std::fmt;
use std::io;
use std::mem;
use std::net::Shutdown;
use std::os::raw::c_int;
use std::time::Duration;

use crate::sys::bt::btc as c;
use crate::sys::bt::Socket;
use crate::sys::bt::{cvt, cvt_r};
use crate::sys_common::AsInner;

use crate::bt::{BtAddr, BtProtocol};

cfg_if! {
    if #[cfg(any(
            target_os = "linux", target_os = "android",
            target_os = "dragonfly", target_os = "freebsd",
            target_os = "openbsd", target_os = "netbsd",
            target_os = "haiku", target_os = "bitrig"
        ))] {
        use libc::MSG_NOSIGNAL;
    } else {
        const MSG_NOSIGNAL: c_int = 0x0;
    }
}

////////////////////////////////////////////////////////////////////////////////
// sockaddr and misc bindings
////////////////////////////////////////////////////////////////////////////////

pub fn setsockopt<T>(sock: &Socket, opt: c_int, val: c_int, payload: T) -> io::Result<()> {
    let payload = &payload as *const T as *const _;
    cvt(unsafe {
        c::setsockopt(
            *sock.as_inner(),
            opt,
            val,
            payload,
            mem::size_of::<T>() as c::socklen_t,
        )
    })?;
    Ok(())
}

pub fn getsockopt<T: Copy>(sock: &Socket, opt: c_int, val: c_int) -> io::Result<T> {
    unsafe {
        let mut slot: T = mem::zeroed();
        let mut len = mem::size_of::<T>() as c::socklen_t;
        cvt(c::getsockopt(
            *sock.as_inner(),
            opt,
            val,
            &mut slot as *mut _ as *mut _,
            &mut len,
        ))?;
        assert_eq!(len as usize, mem::size_of::<T>());
        Ok(slot)
    }
}

fn sockname<F>(f: F) -> io::Result<BtAddr>
where
    F: FnOnce(*mut c::sockaddr_storage, *mut c::socklen_t) -> c_int,
{
    let mut addr: c::sockaddr_storage = unsafe { mem::zeroed() };
    let mut len = mem::size_of_val(&addr) as c::socklen_t;
    cvt(f(&mut addr, &mut len))?;
    Ok((&addr).into())
}

////////////////////////////////////////////////////////////////////////////////
// Bluetooth listeners
////////////////////////////////////////////////////////////////////////////////

pub struct BtListener {
    inner: Socket,
    protocol: BtProtocol,
}

impl BtListener {
    pub fn bind(addr: &BtAddr, protocol: BtProtocol) -> io::Result<Self> {
        let socket = Socket::new(protocol)?;

        // On platforms with Berkeley-derived sockets, this allows
        // to quickly rebind a socket, without needing to wait for
        // the OS to clean up the previous one.
        if !cfg!(windows) {
            setsockopt(&socket, c::SOL_SOCKET, c::SO_REUSEADDR, 1 as c_int)?;
        }

        let (addr, len) = addr.into();
        cvt(unsafe { c::bind(*socket.as_inner(), &addr as *const _ as *const _, len) })?;
        cvt(unsafe { c::listen(*socket.as_inner(), 128) })?;
        Ok(Self {
            inner: socket,
            protocol,
        })
    }

    pub fn accept(&self) -> io::Result<(BtStream, BtAddr)> {
        self.inner.accept().map(|(socket, addr)| {
            (
                BtStream {
                    inner: socket,
                    protocol: self.protocol,
                },
                addr,
            )
        })
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }

    pub fn local_addr(&self) -> io::Result<BtAddr> {
        sockname(|addr, len| unsafe { c::getsockname(*self.inner.as_inner(), addr as *mut _, len) })
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    pub fn protocol(&self) -> BtProtocol {
        self.protocol
    }

    pub fn socket(&self) -> &Socket {
        &self.inner
    }

    pub fn into_socket(self) -> Socket {
        self.inner
    }

    pub fn duplicate(&self) -> io::Result<Self> {
        self.inner.duplicate().map(|s| Self {
            inner: s,
            protocol: self.protocol,
        })
    }
}

impl fmt::Debug for BtListener {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut res = f.debug_struct("BtListener");

        if let Ok(addr) = self.local_addr() {
            res.field("addr", &addr);
        }

        let name = if cfg!(windows) { "socket" } else { "fd" };
        res.field(name, &self.inner.as_inner()).finish()
    }
}

////////////////////////////////////////////////////////////////////////////////
// Bluetooth streams
////////////////////////////////////////////////////////////////////////////////

pub struct BtStream {
    inner: Socket,
    protocol: BtProtocol,
}

impl BtStream {
    pub fn connect(addr: &BtAddr, protocol: BtProtocol) -> io::Result<Self> {
        let (addr, len) = addr.into();

        let socket = Socket::new(protocol)?;
        cvt_r(|| unsafe { c::connect(*socket.as_inner(), &addr as *const _ as *const _, len) })?;
        Ok(Self {
            inner: socket,
            protocol,
        })
    }

    pub fn connect_timeout(
        addr: &BtAddr,
        protocol: BtProtocol,
        timeout: Duration,
    ) -> io::Result<Self> {
        let socket = Socket::new(protocol)?;
        socket.connect_timeout(addr, timeout)?;
        Ok(Self {
            inner: socket,
            protocol,
        })
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.peek(buf)
    }

    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, BtAddr)> {
        self.inner.peek_from(buf)
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, BtAddr)> {
        self.inner.recv_from(buf)
    }

    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        cvt(unsafe {
            c::send(
                *self.inner.as_inner(),
                buf.as_ptr() as *const _,
                cmp::min(buf.len(), <c::wrlen_t>::max_value() as usize) as c::wrlen_t,
                MSG_NOSIGNAL,
            )
        })
        .map(|ret| ret as usize)
    }

    pub fn send_to(&self, buf: &[u8], dst: &BtAddr) -> io::Result<usize> {
        let (addr, addrlen) = dst.into();
        cvt(unsafe {
            c::sendto(
                *self.inner.as_inner(),
                buf.as_ptr() as *const _,
                cmp::min(buf.len(), <c::wrlen_t>::max_value() as usize) as c::wrlen_t,
                MSG_NOSIGNAL,
                &addr as *const _ as *const _,
                addrlen,
            )
        })
        .map(|ret| ret as usize)
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.timeout(c::SO_RCVTIMEO)
    }

    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_timeout(dur, c::SO_RCVTIMEO)
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.timeout(c::SO_SNDTIMEO)
    }

    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_timeout(dur, c::SO_SNDTIMEO)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }

    pub fn local_addr(&self) -> io::Result<BtAddr> {
        sockname(|addr, len| unsafe { c::getsockname(*self.inner.as_inner(), addr as *mut _, len) })
    }

    pub fn peer_addr(&self) -> io::Result<BtAddr> {
        sockname(|addr, len| unsafe { c::getpeername(*self.inner.as_inner(), addr as *mut _, len) })
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    pub fn protocol(&self) -> BtProtocol {
        self.protocol
    }

    pub fn socket(&self) -> &Socket {
        &self.inner
    }

    pub fn into_socket(self) -> Socket {
        self.inner
    }

    pub fn duplicate(&self) -> io::Result<Self> {
        self.inner.duplicate().map(|s| Self {
            inner: s,
            protocol: self.protocol,
        })
    }
}

impl fmt::Debug for BtStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut res = f.debug_struct("BtStream");

        if let Ok(addr) = self.local_addr() {
            res.field("addr", &addr);
        }

        let name = if cfg!(windows) { "socket" } else { "fd" };
        res.field(name, &self.inner.as_inner()).finish()
    }
}
