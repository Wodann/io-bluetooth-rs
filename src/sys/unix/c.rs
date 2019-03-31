pub use libbluetooth::{
    bluetooth::{bdaddr_t, BTPROTO_L2CAP, BTPROTO_RFCOMM},
    hci::{inquiry_info, IREQ_CACHE_FLUSH},
    hci_lib::{hci_close_dev, hci_get_route, hci_inquiry, hci_open_dev},
    rfcomm::sockaddr_rc,
};
pub use libc::{
    accept, bind, connect, getpeername, getsockname, shutdown, sockaddr, socket, socklen_t,
    AF_BLUETOOTH, SHUT_RDWR, SOCK_STREAM,
};
