use std::fmt;
use std::io;
use std::net::Shutdown;
use std::time::Duration;

use crate::sys_common::bt as bt_imp;
use crate::sys_common::{AsInner, FromInner, IntoInner};

/// A Bluetooth address, consisting of 6 bytes.
#[derive(Clone)]
pub struct BtAddr(pub [u8; 6]);

impl BtAddr {
    pub fn nap_sap(nap: u16, sap: u32) -> BtAddr {
        let nap = nap.to_le_bytes();
        let sap = sap.to_le_bytes();
        Self([sap[0], sap[1], sap[2], sap[3], nap[0], nap[1]])
    }
}

impl fmt::Debug for BtAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BtAddr({:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x})",
            self.0[5], self.0[4], self.0[3], self.0[2], self.0[1], self.0[0]
        )
    }
}

impl fmt::Display for BtAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[5], self.0[4], self.0[3], self.0[2], self.0[1], self.0[0]
        )
    }
}

#[derive(Clone, Copy)]
pub enum BtProtocol {
    L2CAP,
    RFCOMM,
}

pub use crate::sys::bt::discover_devices;

/// A Bluetooth socket.
pub struct BtSocket(bt_imp::BtSocket);

impl BtSocket {
    /// Creates a Bluetooth socket from the given address.
    ///
    /// If `addrs` yields multiple addresses, `bind` will be attempted with
    /// each of the addresses until one succeeds and returns the socket. If none
    /// of the addresses succeed in creating a socket, the error returned from
    /// the last attempt (the last address) is returned.
    pub fn bind<'a, I>(addrs: I, protocol: BtProtocol) -> io::Result<BtSocket>
    where
        I: Iterator<Item = &'a BtAddr>,
    {
        each_addr(addrs, |addr| bt_imp::BtSocket::bind(addr, protocol))
            .map(|socket| BtSocket(socket))
    }

    /// Connects this Bluetooth socket to a remote address, allowing the `send` and
    /// `recv` syscalls to be used to send data and also applies filters to only
    /// receive data from the specified address.
    ///
    /// If `addrs` yields multiple addresses, `connect` will be attempted with
    /// each of the addresses until the underlying OS function returns no
    /// error. Note that usually, a successful `connect` call does not specify
    /// that there is a remote server listening on the port, rather, such an
    /// error would only be detected after the first send. If the OS returns an
    /// error for each of the specified addresses, the error returned from the
    /// last connection attempt (the last address) is returned.
    pub fn connect<'a, I: Iterator<Item = &'a BtAddr>>(
        addrs: I,
        protocol: BtProtocol,
    ) -> io::Result<BtSocket> {
        each_addr(addrs, |addr| bt_imp::BtSocket::connect(addr, protocol))
            .map(|socket| BtSocket(socket))
    }

    /// Receives single Bluetooth on the socket from the remote address to which it is
    /// connected, without removing the message from input queue. On success, returns
    /// the number of bytes peeked.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to
    /// hold the message bytes. If a message is too long to fit in the supplied buffer,
    /// excess bytes may be discarded.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recv` system call.
    ///
    /// Do not use this function to implement busy waiting, instead use `libc::poll` to
    /// synchronize IO events on one or more sockets.
    ///
    /// The [`connect`] method will connect this socket to a remote address. This
    /// method will fail if the socket is not connected.
    ///
    /// [`connect`]: #method.connect
    ///
    /// # Errors
    ///
    /// This method will fail if the socket is not connected. The `connect` method
    /// will connect this socket to a remote address.
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.peek(buf)
    }

    /// Receives a single Bluetooth message on the socket, without removing it from the
    /// queue. On success, returns the number of bytes read and the origin.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to
    /// hold the message bytes. If a message is too long to fit in the supplied buffer,
    /// excess bytes may be discarded.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recvfrom` system call.
    ///
    /// Do not use this function to implement busy waiting, instead use `libc::poll` to
    /// synchronize IO events on one or more sockets.
    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, BtAddr)> {
        self.0.peek_from(buf)
    }

    /// Receives a single Bluetooth message on the socket from the remote address to
    /// which it is connected. On success, returns the number of bytes read.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to
    /// hold the message bytes. If a message is too long to fit in the supplied buffer,
    /// excess bytes may be discarded.
    ///
    /// The [`connect`] method will connect this socket to a remote address. This
    /// method will fail if the socket is not connected.
    ///
    /// [`connect`]: #method.connect
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(buf)
    }

    /// Receives a single Bluetooth message on the socket. On success, returns the number
    /// of bytes read and the origin.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to
    /// hold the message bytes. If a message is too long to fit in the supplied buffer,
    /// excess bytes may be discarded.
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, BtAddr)> {
        self.0.recv_from(buf)
    }

    /// Sends data on the socket to the remote address to which it is connected.
    ///
    /// The [`connect`] method will connect this socket to a remote address. This
    /// method will fail if the socket is not connected.
    ///
    /// [`connect`]: #method.connect
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf)
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    pub fn send_to(&self, buf: &[u8], dst: &BtAddr) -> io::Result<usize> {
        self.0.send_to(buf, dst)
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.shutdown(how)
    }

    /// Returns the read timeout of this socket.
    ///
    /// If the timeout is [`None`], then [`read`] calls will block indefinitely.
    ///
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    /// [`read`]: ../../std/io/trait.Read.html#tymethod.read
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.read_timeout()
    }

    /// Sets the read timeout to the timeout specified.
    ///
    /// If the value specified is [`None`], then [`read`] calls will block
    /// indefinitely. An [`Err`] is returned if the zero [`Duration`] is
    /// passed to this method.
    ///
    /// # Platform-specific behavior
    ///
    /// Platforms may return a different error code whenever a read times out as
    /// a result of setting this option. For example Unix typically returns an
    /// error of the kind [`WouldBlock`], but Windows may return [`TimedOut`].
    ///
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    /// [`Err`]: ../../std/result/enum.Result.html#variant.Err
    /// [`read`]: ../../std/io/trait.Read.html#tymethod.read
    /// [`Duration`]: ../../std/time/struct.Duration.html
    /// [`WouldBlock`]: ../../std/io/enum.ErrorKind.html#variant.WouldBlock
    /// [`TimedOut`]: ../../std/io/enum.ErrorKind.html#variant.TimedOut
    ///
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this
    /// method.
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(dur)
    }

    /// Returns the write timeout of this socket.
    ///
    /// If the timeout is [`None`], then [`write`] calls will block indefinitely.
    ///
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    /// [`write`]: ../../std/io/trait.Write.html#tymethod.write
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.write_timeout()
    }

    /// Sets the write timeout to the timeout specified.
    ///
    /// If the value specified is [`None`], then [`write`] calls will block
    /// indefinitely. An [`Err`] is returned if the zero [`Duration`] is
    /// passed to this method.
    ///
    /// # Platform-specific behavior
    ///
    /// Platforms may return a different error code whenever a write times out
    /// as a result of setting this option. For example Unix typically returns
    /// an error of the kind [`WouldBlock`], but Windows may return [`TimedOut`].
    ///
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    /// [`Err`]: ../../std/result/enum.Result.html#variant.Err
    /// [`write`]: ../../std/io/trait.Write.html#tymethod.write
    /// [`Duration`]: ../../std/time/struct.Duration.html
    /// [`WouldBlock`]: ../../std/io/enum.ErrorKind.html#variant.WouldBlock
    /// [`TimedOut`]: ../../std/io/enum.ErrorKind.html#variant.TimedOut
    ///
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this
    /// method.
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(dur)
    }

    /// Moves this Bluetooth socket into or out of nonblocking mode.
    ///
    /// This will result in `recv`, `recv_from`, `send`, and `send_to`
    /// operations becoming nonblocking, i.e., immediately returning from their
    /// calls. If the IO operation is successful, `Ok` is returned and no
    /// further action is required. If the IO operation could not be completed
    /// and needs to be retried, an error with kind
    /// [`io::ErrorKind::WouldBlock`] is returned.
    ///
    /// On Unix platforms, calling this method corresponds to calling `fcntl`
    /// `FIONBIO`. On Windows calling this method corresponds to calling
    /// `ioctlsocket` `FIONBIO`.
    ///
    /// [`io::ErrorKind::WouldBlock`]: ../io/enum.ErrorKind.html#variant.WouldBlock
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }

    /// Returns the socket address that this socket was created from.
    pub fn local_addr(&self) -> io::Result<BtAddr> {
        self.0.local_addr()
    }

    /// Returns the socket address of the remote peer this socket was connected to.
    pub fn peer_addr(&self) -> io::Result<BtAddr> {
        self.0.peer_addr()
    }

    /// Returns the socket protocol of this socket.
    pub fn protocol(&self) -> BtProtocol {
        self.0.protocol()
    }

    /// Gets the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.0.take_error()
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UdpSocket` is a reference to the same socket that this
    /// object references. Both handles will read and write the same port, and
    /// options set on one socket will be propagated to the other.
    pub fn try_clone(&self) -> io::Result<BtSocket> {
        self.0.duplicate().map(BtSocket)
    }
}

impl AsInner<bt_imp::BtSocket> for BtSocket {
    fn as_inner(&self) -> &bt_imp::BtSocket {
        &self.0
    }
}

impl FromInner<bt_imp::BtSocket> for BtSocket {
    fn from_inner(inner: bt_imp::BtSocket) -> BtSocket {
        BtSocket(inner)
    }
}

impl IntoInner<bt_imp::BtSocket> for BtSocket {
    fn into_inner(self) -> bt_imp::BtSocket {
        self.0
    }
}

impl fmt::Debug for BtSocket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

fn each_addr<'a, I, F, T>(addrs: I, mut f: F) -> io::Result<T>
where
    F: FnMut(&'a BtAddr) -> io::Result<T>,
    I: Iterator<Item = &'a BtAddr>,
{
    let mut last_err = None;
    for addr in addrs {
        match f(addr) {
            Ok(l) => return Ok(l),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "could not resolve to any addresses",
        )
    }))
}
