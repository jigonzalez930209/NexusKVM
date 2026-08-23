---
layout: home

hero:
  name: NexusKVM
  text: Ultra-Low Latency Software KVM for Linux
  tagline: Seamlessly share your keyboard and mouse between multiple computers with Wayland, Tauri 2, Rust, and the power of rkvm.
  image:
    src: /logo.svg
    alt: NexusKVM Logo
  actions:
    - theme: brand
      text: 🚀 Get Started
      link: /guide/getting-started
    - theme: alt
      text: 🛡️ Permissions & Security
      link: /guide/permissions-and-security
    - theme: alt
      text: 📦 rkvm Fork & Citations
      link: /architecture/rkvm-integration

features:
  - icon: ⚡
    title: Native Rust Performance
    details: Imperceptible input latency powered by the optimized rkvm-net binary protocol and direct evdev / uinput descriptor handling.
  - icon: 🛡️
    title: Strict Security & Privacy
    details: End-to-end TLS 1.3 encryption, strict privilege separation without running the GUI as root, and full protection against keyloggers.
  - icon: 🖥️
    title: Native Wayland & X11 Support
    details: First-class integration with org.freedesktop.portal.InputCapture and libei/reis, supporting GNOME 46+, KDE Plasma 6, and X11 environments.
  - icon: 🔄
    title: Intelligent Target Routing
    details: Advanced rkvm 0.6.1 fork featuring automatic fail-safe return to local on disconnect and atomic release of retained keystrokes.
  - icon: 🎯
    title: Modern Tauri 2 + React UI
    details: Elegant desktop interface with system tray minimization, 1-click pairing code generation, and visual multi-screen layout topology.
  - icon: 🐧
    title: Login Screen & Boot Support (GDM)
    details: Capable of running as a systemd system service to control the mouse and type passwords on the Linux login screen before user login.
---

<div class="home-custom-section">
  <TopologyDiagram />
  <RkvmCitation />
</div>

<style>
.home-custom-section {
  max-width: 1152px;
  margin: 0 auto;
  padding: 0 24px 64px;
}
</style>
