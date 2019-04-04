pub use winapi::shared::bthdef::{
    GET_NAP, GET_SAP, L2CAP_PROTOCOL_UUID, RFCOMM_PROTOCOL_UUID, SET_NAP_SAP,
};
pub use winapi::shared::guiddef::GUID;
pub use winapi::shared::winerror::{WSAEFAULT, WSA_E_NO_MORE};
pub use winapi::shared::ws2def::{CSADDR_INFO, SOCKADDR, SOCKADDR_STORAGE_LH};
pub use winapi::um::handleapi::SetHandleInformation;
pub use winapi::um::processthreadsapi::GetCurrentProcessId;
pub use winapi::um::winbase::{HANDLE_FLAG_INHERIT, INFINITE};
pub use winapi::um::winnt::HANDLE;
pub use winapi::um::winsock2::{
    accept, bind, closesocket, connect, fd_set, getpeername, getsockname, getsockopt, ioctlsocket,
    recv, recvfrom, select, send, sendto, setsockopt, shutdown, timeval, WSACleanup,
    WSADuplicateSocketW, WSAGetLastError, WSALookupServiceBeginW, WSALookupServiceEnd,
    WSALookupServiceNextW, WSASocketW, WSAStartup, FIONBIO, INVALID_SOCKET, LUP_CONTAINERS,
    LUP_FLUSHCACHE, LUP_RETURN_ADDR, MSG_PEEK, NS_BTH, SD_BOTH, SD_RECEIVE, SD_SEND, SOCKET,
    SOCKET_ERROR, SOCK_STREAM, SOL_SOCKET, SO_ERROR, SO_RCVTIMEO, SO_SNDTIMEO, WSADATA,
    WSAESHUTDOWN, WSAPROTOCOL_INFOW, WSAQUERYSETW, WSA_FLAG_OVERLAPPED,
};
pub use winapi::um::ws2bth::{AF_BTH, BTHPROTO_L2CAP, BTHPROTO_RFCOMM, BT_PORT_ANY, SOCKADDR_BTH};
