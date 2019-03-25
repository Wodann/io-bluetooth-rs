use std::io;
use std::mem;
use std::sync::Once;

use crate::sys::c;

use crate::{BtAddr, BtProtocol};

/// Checks whether the Windows socket interface has been started already, and
/// if not, starts it.
/// TODO: Does not account for c::WSACleanup()
fn init() {
    static START: Once = Once::new();

    START.call_once(|| unsafe {
        let mut data: c::WSADATA = mem::zeroed();
        let ret = c::WSAStartup(
            0x202, // version 2.2
            &mut data,
        );
        assert_eq!(ret, 0);
    });
}

/// Returns the last error from the Windows socket interface.
fn last_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { c::WSAGetLastError() })
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
                &mut query as *mut c::WSAQUERYSETW,
                c::LUP_CONTAINERS | c::LUP_FLUSHCACHE,
                &mut handle as *mut c::HANDLE,
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
                &mut len as *mut u32,
                query,
            ) {
                let query: c::WSAQUERYSETW = *query;
                let addr_info: c::CSADDR_INFO = *query.lpcsaBuffer;
                let addr: c::SOCKADDR_BTH =
                    *(addr_info.RemoteAddr.lpSockaddr as *mut c::SOCKADDR_BTH);
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

pub struct Socket(c::SOCKET);

pub struct BtSocket {
    inner: Socket,
    protocol: BtProtocol,
}

impl BtSocket {
    pub fn new(protocol: BtProtocol) -> io::Result<Self> {
        init();

        let socket = {
            let protocol = match protocol {
                BtProtocol::L2CAP => c::BTHPROTO_L2CAP,
                BtProtocol::RFCOMM => c::BTHPROTO_RFCOMM,
            };
            unsafe { c::socket(c::AF_BTH as i32, c::SOCK_STREAM, protocol as i32) }
        };

        if socket == c::INVALID_SOCKET {
            Err(last_error())
        } else {
            Ok(Self {
                inner: Socket(socket),
                protocol,
            })
        }
    }

    pub fn accept(&mut self) -> io::Result<Socket> {
        let socket = unsafe { c::accept(self.inner.0, std::ptr::null_mut(), std::ptr::null_mut()) };
        if socket != c::INVALID_SOCKET {
            Ok(Socket(socket))
        } else {
            Err(last_error())
        }
    }

    pub fn bind(&mut self, address: BtAddr) -> io::Result<()> {
        let sab = c::SOCKADDR_BTH {
            addressFamily: c::AF_BTH,
            btAddr: address.into(),
            serviceClassId: protocol_guid(self.protocol),
            ..Default::default()
        };

        let res = unsafe {
            c::bind(
                self.inner.0,
                &sab as *const c::SOCKADDR_BTH as *const c::SOCKADDR,
                mem::size_of::<c::SOCKADDR_BTH>() as i32,
            )
        };
        if res != c::SOCKET_ERROR {
            Ok(())
        } else {
            Err(last_error())
        }
    }

    pub fn connect(&mut self, address: BtAddr) -> io::Result<()> {
        let sab = c::SOCKADDR_BTH {
            addressFamily: c::AF_BTH,
            btAddr: address.into(),
            serviceClassId: protocol_guid(self.protocol),
            ..Default::default()
        };

        let res = unsafe {
            c::connect(
                self.inner.0,
                &sab as *const c::SOCKADDR_BTH as *const c::SOCKADDR,
                mem::size_of::<c::SOCKADDR_BTH>() as i32,
            )
        };
        if res != c::SOCKET_ERROR {
            Ok(())
        } else {
            Err(last_error())
        }
    }

    pub fn peername(&self) -> io::Result<BtAddr> {
        let mut sab: c::SOCKADDR_BTH = Default::default();
        let mut len = mem::size_of::<c::SOCKADDR_BTH>() as i32;

        let res = unsafe {
            c::getpeername(
                self.inner.0,
                &mut sab as *mut c::SOCKADDR_BTH as *mut c::SOCKADDR,
                &mut len as *mut i32,
            )
        };
        if res != c::SOCKET_ERROR {
            Ok(BtAddr::nap_sap(
                c::GET_NAP(sab.btAddr),
                c::GET_SAP(sab.btAddr),
            ))
        } else {
            Err(last_error())
        }
    }

    pub fn sockname(&self) -> io::Result<BtAddr> {
        let mut sab: c::SOCKADDR_BTH = Default::default();
        let mut len = mem::size_of::<c::SOCKADDR_BTH>() as i32;

        let res = unsafe {
            c::getsockname(
                self.inner.0,
                &mut sab as *mut c::SOCKADDR_BTH as *mut c::SOCKADDR,
                &mut len as *mut i32,
            )
        };
        if res != c::SOCKET_ERROR {
            Ok(BtAddr::nap_sap(
                c::GET_NAP(sab.btAddr),
                c::GET_SAP(sab.btAddr),
            ))
        } else {
            Err(last_error())
        }
    }

    pub fn socket(&self) -> &Socket {
        &self.inner
    }

    pub fn into_socket(self) -> Socket {
        self.inner
    }
}

fn protocol_guid(protocol: BtProtocol) -> c::GUID {
    match protocol {
        BtProtocol::L2CAP => c::L2CAP_PROTOCOL_UUID,
        BtProtocol::RFCOMM => c::RFCOMM_PROTOCOL_UUID,
    }
}

impl io::Read for BtSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let res = unsafe {
            c::recv(
                self.inner.0,
                buf.as_mut_ptr() as *mut i8,
                buf.len() as i32,
                0,
            )
        };
        if res != c::SOCKET_ERROR {
            Ok(res as usize)
        } else {
            Err(last_error())
        }
    }
}

impl io::Write for BtSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let res = unsafe { c::send(self.inner.0, buf.as_ptr() as *mut i8, buf.len() as i32, 0) };
        if res != c::SOCKET_ERROR {
            Ok(res as usize)
        } else {
            Err(last_error())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        // Winsock automatically flushes the written buffer
        Ok(())
    }
}

impl Into<u64> for BtAddr {
    fn into(self) -> u64 {
        let sap = u32::from_le_bytes([self.0[0], self.0[1], self.0[2], self.0[3]]);
        let nap = u16::from_le_bytes([self.0[4], self.0[5]]);
        c::SET_NAP_SAP(nap, sap)
    }
}
