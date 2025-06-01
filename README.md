# Bird-lg-rs

A Rust implementation of [bird-lg-go](https://github.com/xddxdd/bird-lg-go), providing a complete Bird Looking Glass solution. This project is designed to be a drop-in replacement for bird-lg-go with identical functionality and API compatibility.

## Features

- **Complete compatibility** with bird-lg-go
- **Frontend and Proxy separation** - same architecture as the original
- **All original features**:
  - BGP protocol status display
  - Route queries and filtering
  - Traceroute functionality
  - Whois queries
  - BGP path visualization (bgpmap)
  - REST API endpoints
  - Telegram bot webhook support
- **Performance improvements** through Rust's efficiency
- **Memory safety** and reliability

## Build Instructions

You need to have **Rust 1.70 or newer** installed on your machine.

Run `make` to build binaries for both the frontend and the proxy.

Optionally run `make install` to install them to `/usr/local/bin` (`bird-lg-rs` and `bird-lgproxy-rs`).

### Build Individual Components

```bash
# Build frontend only
make frontend

# Build proxy only  
make proxy

# Clean build artifacts
make clean
```

## Frontend

The frontend provides the web interface where users can view BGP states, perform traceroutes, whois queries, etc.

### Configuration

All configuration options are identical to bird-lg-go:

| Config Key | Parameter | Environment Variable | Description |
| ---------- | --------- | -------------------- | ----------- |
| servers | --servers | BIRDLG_SERVERS | server name prefixes, separated by comma |
| domain | --domain | BIRDLG_DOMAIN | server name domain suffixes |
| listen | --listen | BIRDLG_LISTEN | address bird-lg is listening on (default "5000") |
| proxy_port | --proxy-port | BIRDLG_PROXY_PORT | port bird-lgproxy is running on (default 8000) |
| whois | --whois | BIRDLG_WHOIS | whois server for queries (default "whois.verisign-grs.com") |
| dns_interface | --dns-interface | BIRDLG_DNS_INTERFACE | dns zone to query ASN information (default "asn.cymru.com") |
| bgpmap_info | --bgpmap-info | BIRDLG_BGPMAP_INFO | the infos displayed in bgpmap, separated by comma (default "asn,as-name,ASName,descr") |
| title_brand | --title-brand | BIRDLG_TITLE_BRAND | prefix of page titles in browser tabs (default "Bird-lg Rust") |
| navbar_brand | --navbar-brand | BIRDLG_NAVBAR_BRAND | brand to show in the navigation bar (default "Bird-lg Rust") |
| navbar_brand_url | --navbar-brand-url | BIRDLG_NAVBAR_BRAND_URL | the url of the brand to show in the navigation bar (default "/") |
| navbar_all_servers | --navbar-all-servers | BIRDLG_NAVBAR_ALL_SERVERS | the text of "All servers" button in the navigation bar (default "ALL Servers") |
| navbar_all_url | --navbar-all-url | BIRDLG_NAVBAR_ALL_URL | the URL of "All servers" button (default "all") |
| net_specific_mode | --net-specific-mode | BIRDLG_NET_SPECIFIC_MODE | apply network-specific changes for some networks |
| protocol_filter | --protocol-filter | BIRDLG_PROTOCOL_FILTER | protocol types to show in summary tables (comma separated list) |
| name_filter | --name-filter | BIRDLG_NAME_FILTER | protocol names to hide in summary tables (RE2 syntax) |
| timeout | --timeout | BIRDLG_TIMEOUT | time before request timed out, in seconds (default 120) |

### Example

```bash
./bird-lg-rs --servers=server1,server2 --domain=example.com --proxy-port=8000
```

## Proxy

The proxy provides the backend API for BIRD commands and traceroute functionality.

### Configuration

All configuration options are identical to bird-lg-go:

| Config Key | Parameter | Environment Variable | Description |
| ---------- | --------- | -------------------- | ----------- |
| allowed | --allowed | ALLOWED_IPS | IPs or networks allowed to access this proxy, separated by commas |
| bird | --bird | BIRD_SOCKET | socket file for bird (default "/var/run/bird/bird.ctl") |
| listen | --listen | BIRDLG_PROXY_PORT | listen address (default "8000") |
| traceroute_bin | --traceroute-bin | BIRDLG_TRACEROUTE_BIN | traceroute binary file |
| traceroute_flags | --traceroute-flags | BIRDLG_TRACEROUTE_FLAGS | traceroute flags, supports multiple flags separated with space |
| traceroute_raw | --traceroute-raw | BIRDLG_TRACEROUTE_RAW | whether to display traceroute outputs raw (default false) |

### Example

```bash
./bird-lgproxy-rs --bird /run/bird.ctl --listen 8000
```

## Migration from bird-lg-go

This project is designed to be a **drop-in replacement** for bird-lg-go. Simply:

1. Stop your existing bird-lg-go services
2. Replace the binaries with `bird-lg-rs` and `bird-lgproxy-rs`
3. Start the services with the same configuration

All command-line arguments, environment variables, and API endpoints remain identical.

## API Compatibility

All REST API endpoints are fully compatible with bird-lg-go:

- `/api/bird/:servers/:command`
- `/api/traceroute/:servers/:target`
- `/api/whois/:target`

## License

GPL 3.0 - Same as bird-lg-go

## Credits

- [bird-lg-go](https://github.com/xddxdd/bird-lg-go) - The original Go implementation
- [bird-lg](https://github.com/sileht/bird-lg) - The original Python implementation
- All contributors to the bird-lg ecosystem