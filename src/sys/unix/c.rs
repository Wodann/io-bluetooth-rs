pub use libbluetooth::{
    bdaddr_t, hci_close_dev, hci_get_route, hci_inquiry, hci_open_dev, inquiry_info, sockaddr_rc,
    BTPROTO_L2CAP, BTPROTO_RFCOMM, IREQ_CACHE_FLUSH,
};
pub use libc::{
    accept, bind, connect, getpeername, getsockname, shutdown, sockaddr, socket, socklen_t,
    AF_BLUETOOTH, SHUT_RDWR, SOCK_STREAM,
};
