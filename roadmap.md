# VuIO Roadmap

## Vision & Mission

The main idea of **VuIO** is to be the best **open-source** and **zero-ads** media server with extremely low resource usage and a highly modular architecture.

VuIO is designed for offline and localized media sharing in environments such as:
- Homes & vacation homes
- Schools & universities
- Cruise ships & yachts
- Buses. plains, trains
- Remote outposts & bunkers
- Polar research stations
- Future lunar & Mars bases
- Any location where commercial streaming services (e.g., Netflix) are out of reach or where high-volume local media content needs to be shared over the local networks.

---

## Core Architecture & Ecosystem

- **`vuio-core` (Main Library)**
  - Designed for integration and embedding by third-party vendors (NAS devices, routers, SBCs, traditional servers, Docker / Helm deployments).
  - High modularity enables external projects and software to use the `vuio` library as their foundation.
  - Planned progression toward a stable API and official publication on [crates.io](https://crates.io/).

- **CLI (`vuio-cli`)**
  - Separate CLI application (currently bundled with `vuio-core`).
  - Manages local servers today, with planned support for managing remote servers in the future.

- **`vuio-tower` (GUI Management Tool)**
  - Graphical tool to control VuIO servers from Mac/Windows/Linux tray.
  - Currently controls local VuIO servers, with planned expansion to manage remote servers.
  https://github.com/vuiodev/vuio-tower

---

## Short-Term Plans

- [x] Stabilize the api for vuio-core
- [x] Publish it on crates.io

---

## Long-Term Wishlist

### Client Applications
- [ ] **Android / iOS** phone and tablet app with DirectPlay support over HTTPS connection
- [ ] **Android TV** app
- [ ] **Samsung TV** app
- [ ] **LG TV** app
- [ ] **Apple TV** app
- [ ] **Amazon Fire TV** app