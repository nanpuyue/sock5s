# sock5s

A lightweight SOCKS5 proxy server written in Rust.

## Features

- ✅ RFC 1928 compatible
- ✅ NO AUTHENTICATION REQUIRED
- ✅ CONNECT command
  - ✅ IPv4
  - ✅ IPv6
  - ✅ Domain
- ✅ UDP ASSOCIATE command
  - ✅ IPv4
  - ✅ IPv6
  - ✅ Domain
- ✅ Dual-stack (IPv4 / IPv6) support
- ✅ Asynchronous implementation based on Tokio
- ✅ Cross-platform support (Linux / macOS / Windows)

## Usage

```
sock5s 0.3.0
nanpuyue <nanpuyue@gmail.com>
A lightweight SOCKS5 proxy server written in Rust.

Usage: sock5s --listen <HOST:PORT>

Options:
  -l, --listen <HOST:PORT>  Listen address
  -h, --help                Print help
  -V, --version             Print version
```

## License

This project is licensed under the [MIT license].

[MIT license]: https://github.com/nanpuyue/sock5s/blob/master/LICENSE

## Homepage

[https://github.com/nanpuyue/sock5s](https://github.com/nanpuyue/sock5s)
