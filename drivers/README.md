# Drivers

Planned (see ../docs/ARCHITECTURE.md §4.1):

- `virtual-audio/` — kernel WDM/PortCls virtual endpoints, derived from Microsoft SysVAD
- `asio/` — user-mode ASIO driver
- `r-interface/` — R Interface driver / support, pending firmware answers

Prerequisites before starting: Visual Studio 2022 (Desktop C++), matching Windows Driver Kit,
and a dedicated test machine with test-signing enabled.
