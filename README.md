# 🦅 Bird-lg-rs

Another blazing-fast Rust implementation of [bird-lg-go](https://github.com/xddxdd/bird-lg-go), delivering a complete Bird Looking Glass solution with enhanced performance and reliability. This project serves as a drop-in replacement for `bird-lg-go`, maintaining full API compatibility while leveraging Rust's superior performance characteristics.

## ✨ Features

- **🔄 Complete compatibility** with `bird-lg-go` - seamless migration path
- **🏗️ Frontend and Proxy separation** - maintains the proven architecture
- **🚀 All original features included**:
  - BGP protocol status display and monitoring
  - Advanced route queries with filtering capabilities
  - Comprehensive traceroute functionality
  - Integrated whois query system
  - BGP path visualization (bgpmap) with detailed routing information
  - Full REST API endpoints for programmatic access
  - Telegram bot webhook support for notifications
- **⚡ Performance improvements** through Rust's zero-cost abstractions
- **🛡️ Memory safety** and enhanced reliability guarantees
- **🔧 Zero-configuration migration** from existing bird-lg-go deployments

## 🔨 Build Instructions

Ensure you have **Docker Engine** installed on your system.

Execute `./build.sh` to build images for both the frontend and proxy components.

## 🌐 Frontend

The frontend delivers an intuitive web interface enabling users to monitor BGP states, execute traceroutes, perform whois queries, and visualize network topology.

### ⚙️ Configuration

All configuration options maintain complete compatibility with bird-lg-go:

| Config Key | Parameter | Environment Variable | Description |
| ---------- | --------- | -------------------- | ----------- |
| servers | --servers | BIRDLG_SERVERS | server name prefixes, separated by comma |
| domain | --domain | BIRDLG_DOMAIN | server name domain suffixes |
| listen | --listen | BIRDLG_LISTEN | address bird-lg is listening on (default "5000") |
| proxy_port | --proxy-port | BIRDLG_PROXY_PORT | port bird-lgproxy is running on (default 8000) |
| whois | --whois | BIRDLG_WHOIS | whois server for queries (default "whois.dn42") |
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

### 💡 Example Usage

```bash
./bird-lg-rs --servers=server1,server2 --domain=example.com --proxy-port=8000
```

## 🔌 Proxy

The proxy component provides a robust backend API for BIRD commands and comprehensive traceroute functionality, serving as the bridge between the frontend interface and your network infrastructure.

### ⚙️ Configuration

All configuration parameters maintain full compatibility with bird-lg-go:

| Config Key | Parameter | Environment Variable | Description |
| ---------- | --------- | -------------------- | ----------- |
| allowed | --allowed | ALLOWED_IPS | IPs or networks allowed to access this proxy, separated by commas |
| bird | --bird | BIRD_SOCKET | socket file for bird (default "/var/run/bird/bird.ctl") |
| listen | --listen | BIRDLG_PROXY_PORT | listen address (default "8000") |
| traceroute_bin | --traceroute-bin | BIRDLG_TRACEROUTE_BIN | traceroute binary file |
| traceroute_flags | --traceroute-flags | BIRDLG_TRACEROUTE_FLAGS | traceroute flags, supports multiple flags separated with space |
| traceroute_raw | --traceroute-raw | BIRDLG_TRACEROUTE_RAW | whether to display traceroute outputs raw (default false) |

### 💡 Example Usage

```bash
./bird-lgproxy-rs --bird /run/bird.ctl --listen 8000
```

## 🚀 Migration from `bird-lg-go`

This project is engineered as a **seamless drop-in replacement** for bird-lg-go. Migration is straightforward:

1. **Stop** your existing bird-lg-go services
2. **Replace** the binaries with `bird-lg-rs` and `bird-lgproxy-rs`
3. **Start** the services using your existing configuration

All command-line arguments, environment variables, and API endpoints remain completely identical, ensuring zero downtime migration.

## 🔌 API Compatibility

All REST API endpoints maintain full compatibility with bird-lg-go, ensuring existing integrations continue to function seamlessly:

- `/api/bird/:servers/:command` - Execute BIRD commands across specified servers
- `/api/traceroute/:servers/:target` - Perform traceroute operations from multiple vantage points
- `/api/whois/:target` - Query whois information for IP addresses and domains

## 📄 License

GPL 3.0 - Maintaining consistency with the original `bird-lg-go` license

## 🙏 Credits

- [bird-lg-rs](https://github.com/pysio2007/bird-lg-rs) - The original Rust implementation by Pysio
- [bird-lg-go](https://github.com/xddxdd/bird-lg-go) - The original Go implementation that inspired this project
- [bird-lg](https://github.com/sileht/bird-lg) - The foundational Python implementation
- All contributors to the bird-lg ecosystem who have made this project possible

---

Built with ❤️ in Rust for the networking community
