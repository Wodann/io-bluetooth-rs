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

/// A Bluetooth socket server, listening for connections.
///
/// After creating a `BtListener` by [`bind`]ing it to a Bluetooth address, it listens
/// for incoming Bluetooth connections. These can be accepted by calling [`accept`] or by
/// iterating over the [`Incoming`] iterator returned by [`incoming`]
/// [`BtListener::incoming`].
///
/// The socket will be closed when the value is dropped.
///
/// The Bluetooth transport protocols are specified by the
/// [Bluetooth Special Interest Group].
///
/// [`accept`]: #method.accept
/// [`bind`]: #method.bind
/// [Bluetooth Special Interest Group]: https://www.bluetooth.com/specifications
/// [`Incoming`]: https://doc.rust-lang.org/std/net/struct.Incoming.html
/// [`BtListener::incoming`]: #method.incoming
pub struct BtListener(bt_imp::BtListener);

/// A Bluetooth stream between a local and remote socket
///
/// After creating a `BtStream` by either [`connect`]ing to a remote host or [`accept`]ing
/// a connection on a [`BtListener`], data can be transmitted by [reading] and [writing]
/// to it.
///
/// The connection will be closed when the value is dropped. The reading and writing
/// portions of the connection can also be shut down individually with the [`shutdown`]
/// method.
///
/// The Bluetooth transport protocols are specified by the
/// [Bluetooth Special Interest Group].
///
/// [`accept`]: ../struct.BtListener.html#method.accept
/// [Bluetooth Special Interest Group]: https://www.bluetooth.com/specifications
/// [`connect`]: #method.connect
/// [reading]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`shutdown`]: #method.shutdown
/// [`BtListener`]: ../struct.BtListener.html
/// [writing]: https://doc.rust-lang.org/std/io/trait.Write.html
pub struct BtStream(bt_imp::BtStream);

impl BtListener {
    /// Creates a new `BtListener` which will be bound to the specified address.
    ///
    /// The returned listener is ready for accepting connections.
    ///
    /// Binding with a port number of 0 will request that the OS assigns a port to this
    /// listener. The port allocated can be queried via the [`local_addr`] method.
    ///
    /// If `addrs` yields multiple addresses, `bind` will be attempted with each of the
    /// addresses until one succeeds and returns the socket. If none of the addresses
    /// succeed in creating a socket, the error returned from the last attempt (the last
    /// address) is returned.
    ///
    /// [`local_addr`]: #method.local_addr
    pub fn bind<'a, I>(addrs: I, protocol: BtProtocol) -> io::Result<Self>
    where
        I: Iterator<Item = &'a BtAddr>,
    {
        each_addr(addrs, |addr| bt_imp::BtListener::bind(addr, protocol)).map(BtListener)
    }

    /// Accept a new incoming connection from this listener.
    ///
    /// This function will block the calling thread until a new Bluetooth connection is
    /// established. When established, the corresponding [`BtStream`] and the remote
    /// peer's address will be returned.
    ///
    /// [`BtStream`]: bt/struct.BtStream.html
    pub fn accept(&self) -> io::Result<(BtStream, BtAddr)> {
        // On WASM, `TcpStream` is uninhabited (as it's unsupported) and so
        // the `a` variable here is technically unused.
        #[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
        self.0.accept().map(|(a, b)| (BtStream(a), b))
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<BtAddr> {
        self.0.local_addr()
    }

    /// Returns the socket protocol of this socket.
    pub fn protocol(&self) -> BtProtocol {
        self.0.protocol()
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing the field
    /// in the process. This can be useful for checking errors between calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.0.take_error()
    }

    /// Moves this Bluetooth stream into or out of nonblocking mode.
    ///
    /// This will result in the `accept` operation becoming nonblocking, i.e., immediately
    /// returning from their calls. If the IO operation is successful, `Ok` is returned
    /// and no further action is required. If the IO operation could not be completed and
    /// needs to be retried, an error with kind [`io::ErrorKind::WouldBlock`] is returned.
    ///
    /// On Unix platforms, calling this method corresponds to calling `fcntl` `FIONBIO`.
    /// On Windows calling this method corresponds to calling `ioctlsocket` `FIONBIO`.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned [`BtListener`] is a reference to the same socket that this
    /// object references. Both handles can be used to accept incoming
    /// connections and options set on one listener will affect the other.
    ///
    /// [`BtListener`]: bt/struct.BtListener.html
    pub fn try_clone(&self) -> io::Result<BtListener> {
        self.0.duplicate().map(BtListener)
    }
}

impl fmt::Debug for BtListener {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl AsInner<bt_imp::BtListener> for BtListener {
    fn as_inner(&self) -> &bt_imp::BtListener {
        &self.0
    }
}

impl FromInner<bt_imp::BtListener> for BtListener {
    fn from_inner(inner: bt_imp::BtListener) -> BtListener {
        BtListener(inner)
    }
}

impl IntoInner<bt_imp::BtListener> for BtListener {
    fn into_inner(self) -> bt_imp::BtListener {
        self.0
    }
}

impl BtStream {
    /// Opens a Bluetooth connection to a remote host.
    ///
    /// If `addrs` yields multiple addresses, `connect` will be attempted with each of the
    /// addresses until the underlying OS function returns no error. Note that usually, a
    /// successful `connect` call does not specify that there is a remote server listening
    /// on the port, rather, such an error would only be detected after the first send. If
    /// the OS returns an error for each of the specified addresses, the error returned
    /// from the last connection attempt (the last address) is returned.
    pub fn connect<'a, I: Iterator<Item = &'a BtAddr>>(
        addrs: I,
        protocol: BtProtocol,
    ) -> io::Result<Self> {
        each_addr(addrs, |addr| bt_imp::BtStream::connect(addr, protocol)).map(BtStream)
    }

    /// Opens a Bluetooth connection to a remote host with a timeout.
    ///
    /// Unlike `connect`, `connect_timeout` takes a single [`BtAddr`] since timeout must
    /// be applied to individual addresses.
    ///
    /// It is an error to pass a zero `Duration` to this function.
    ///
    /// Unlike other methods on `BtStream`, this does not correspond to a single system
    /// call. It instead calls `connect` in nonblocking mode and then uses an OS-specific
    /// mechanism to await the completion of the connection request.
    ///
    /// [`BtAddr`]: https://doc.rust-lang.org/std/net/enum.BtAddr.html
    pub fn connect_timeout(
        addr: &BtAddr,
        protocol: BtProtocol,
        timeout: Duration,
    ) -> io::Result<Self> {
        bt_imp::BtStream::connect_timeout(addr, protocol, timeout).map(BtStream)
    }

    /// Receives single Bluetooth on the socket from the remote address to which it is
    /// connected, without removing the message from input queue. On success, returns the
    /// number of bytes peeked.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to hold
    /// the message bytes. If a message is too long to fit in the supplied buffer, excess
    /// bytes may be discarded.
    ///
    /// Successive calls return the same data. This is accomplished by passing `MSG_PEEK`
    /// as a flag to the underlying `recv` system call.
    ///
    /// Do not use this function to implement busy waiting, instead use `libc::poll` to
    /// synchronize IO events on one or more sockets.
    ///
    /// The [`connect`] method will connect this socket to a remote address. This method
    /// will fail if the socket is not connected.
    ///
    /// [`connect`]: #method.connect
    ///
    /// # Errors
    ///
    /// This method will fail if the socket is not connected. The `connect` method will
    /// connect this socket to a remote address.
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.peek(buf)
    }

    /// Receives a single Bluetooth message on the socket, without removing it from the
    /// queue. On success, returns the number of bytes read and the origin.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to hold
    /// the message bytes. If a message is too long to fit in the supplied buffer, excess
    /// bytes may be discarded.
    ///
    /// Successive calls return the same data. This is accomplished by passing `MSG_PEEK`
    /// as a flag to the underlying `recvfrom` system call.
    ///
    /// Do not use this function to implement busy waiting, instead use `libc::poll` to
    /// synchronize IO events on one or more sockets.
    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, BtAddr)> {
        self.0.peek_from(buf)
    }

    /// Receives a single Bluetooth message on the socket from the remote address to which
    /// it is connected. On success, returns the number of bytes read.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to hold
    /// the message bytes. If a message is too long to fit in the supplied buffer, excess
    /// bytes may be discarded.
    ///
    /// The [`connect`] method will connect this socket to a remote address. This method
    /// will fail if the socket is not connected.
    ///
    /// [`connect`]: #method.connect
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(buf)
    }

    /// Receives a single Bluetooth message on the socket. On success, returns the number
    /// of bytes read and the origin.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to hold
    /// the message bytes. If a message is too long to fit in the supplied buffer, excess
    /// bytes may be discarded.
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, BtAddr)> {
        self.0.recv_from(buf)
    }

    /// Sends data on the socket to the remote address to which it is connected.
    ///
    /// The [`connect`] method will connect this socket to a remote address. This method
    /// will fail if the socket is not connected.
    ///
    /// [`connect`]: #method.connect
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf)
    }

    /// Sends data on the socket to the given address. On success, returns the number of
    /// bytes written.
    pub fn send_to(&self, buf: &[u8], dst: &BtAddr) -> io::Result<usize> {
        self.0.send_to(buf, dst)
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O on the specified portions to
    /// return immediately with an appropriate value (see the documentation of [`Shutdown`]
    /// ).
    ///
    /// [`Shutdown`]: https://doc.rust-lang.org/std/net/enum.Shutdown.html
    ///
    /// # Platform-specific behavior
    ///
    /// Calling this function multiple times may result in different behavior, depending
    /// on the operating system. On Linux, the second call will return `Ok(())`, but on
    /// macOS, it will return `ErrorKind::NotConnected`. This may change in the future.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.shutdown(how)
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

    /// Returns the read timeout of this socket.
    ///
    /// If the timeout is [`None`], then [`read`] calls will block indefinitely.
    ///
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    /// [`read`]: https://doc.rust-lang.org/std/io/trait.Read.html#tymethod.read
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.read_timeout()
    }

    /// Sets the read timeout to the timeout specified.
    ///
    /// If the value specified is [`None`], then [`read`] calls will block indefinitely.
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this method.
    ///
    /// # Platform-specific behavior
    ///
    /// Platforms may return a different error code whenever a read times out as a result
    /// of setting this option. For example Unix typically returns an error of the kind
    /// [`WouldBlock`], but Windows may return [`TimedOut`].
    ///
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    /// [`Err`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Err
    /// [`read`]: https://doc.rust-lang.org/std/io/trait.Read.html#tymethod.read
    /// [`Duration`]: https://doc.rust-lang.org/std/time/struct.Duration.html
    /// [`WouldBlock`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock
    /// [`TimedOut`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.TimedOut
    ///
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this method.
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(dur)
    }

    /// Returns the write timeout of this socket.
    ///
    /// If the timeout is [`None`], then [`write`] calls will block indefinitely.
    ///
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    /// [`write`]: https://doc.rust-lang.org/std/io/trait.Write.html#tymethod.write
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.write_timeout()
    }

    /// Sets the write timeout to the timeout specified.
    ///
    /// If the value specified is [`None`], then [`write`] calls will block indefinitely.
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this method.
    ///
    /// # Platform-specific behavior
    ///
    /// Platforms may return a different error code whenever a write times out as a result
    /// of setting this option. For example Unix typically returns an error of the kind
    /// [`WouldBlock`], but Windows may return [`TimedOut`].
    ///
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    /// [`Err`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Err
    /// [`write`]: https://doc.rust-lang.org/std/io/trait.Write.html#tymethod.write
    /// [`Duration`]: https://doc.rust-lang.org/std/time/struct.Duration.html
    /// [`WouldBlock`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock
    /// [`TimedOut`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.TimedOut
    ///
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this method.
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(dur)
    }

    /// Moves this Bluetooth socket into or out of nonblocking mode.
    ///
    /// This will result in `recv`, `recv_from`, `send`, and `send_to` operations becoming
    /// nonblocking, i.e., immediately returning from their calls. If the IO operation is
    /// successful, `Ok` is returned and no further action is required. If the IO
    /// operation could not be completed and needs to be retried, an error with kind
    /// [`io::ErrorKind::WouldBlock`] is returned.
    ///
    /// On Unix platforms, calling this method corresponds to calling `fcntl` `FIONBIO`.
    /// On Windows calling this method corresponds to calling `ioctlsocket` `FIONBIO`.
    ///
    /// [`io::ErrorKind::WouldBlock`]: ../io/enum.ErrorKind.html#variant.WouldBlock
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UdpSocket` is a reference to the same socket that this
    /// object references. Both handles will read and write the same port, and
    /// options set on one socket will be propagated to the other.
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.duplicate().map(BtStream)
    }
}

impl fmt::Debug for BtStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl io::Read for BtStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(buf)
    }
}

impl io::Write for BtStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsInner<bt_imp::BtStream> for BtStream {
    fn as_inner(&self) -> &bt_imp::BtStream {
        &self.0
    }
}

impl FromInner<bt_imp::BtStream> for BtStream {
    fn from_inner(inner: bt_imp::BtStream) -> BtStream {
        BtStream(inner)
    }
}

impl IntoInner<bt_imp::BtStream> for BtStream {
    fn into_inner(self) -> bt_imp::BtStream {
        self.0
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

#[cfg(test)]
mod tests {}
