pub use winapi::shared::bthdef::{
    GET_NAP, GET_SAP, L2CAP_PROTOCOL_UUID, RFCOMM_PROTOCOL_UUID, SET_NAP_SAP,
};
pub use winapi::shared::guiddef::GUID;
pub use winapi::shared::winerror::{WSAEFAULT, WSA_E_NO_MORE};
pub use winapi::shared::ws2def::{CSADDR_INFO, SOCKADDR};
pub use winapi::um::winnt::HANDLE;
pub use winapi::um::winsock2::{
    accept, bind, connect, getpeername, getsockname, recv, send, socket, WSACleanup,
    WSAGetLastError, WSALookupServiceBeginW, WSALookupServiceEnd, WSALookupServiceNextW,
    WSAStartup, INVALID_SOCKET, LUP_CONTAINERS, LUP_FLUSHCACHE, LUP_RETURN_ADDR, NS_BTH, SOCKET,
    SOCKET_ERROR, SOCK_STREAM, WSADATA, WSAQUERYSETW,
};
pub use winapi::um::ws2bth::{AF_BTH, BTHPROTO_L2CAP, BTHPROTO_RFCOMM, BT_PORT_ANY, SOCKADDR_BTH};
