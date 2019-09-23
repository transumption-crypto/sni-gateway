Proxy that peeks the socket for TLS Client Hello message without consuming it,
resolves SNI hostname against DNS server and splices socket straight to
the resolved IP address.

[Rustls][] is used for parsing TLS, [Tokio][] is used for handling TCP.

[Rustls]: https://github.com/ctz/rustls
[Tokio]: https://tokio.rs
