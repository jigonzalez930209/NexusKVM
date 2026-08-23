<template>
  <div class="topology-container">
    <div class="topology-header">
      <span class="pulse-status"></span>
      <span class="topology-title">NexusKVM Architecture &amp; Input Flow Topology</span>
    </div>

    <div class="topology-graphic-wrapper">
      <svg viewBox="0 0 800 360" class="topology-svg" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="hostGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="#0284c7" />
            <stop offset="100%" stop-color="#0369a1" />
          </linearGradient>

          <linearGradient id="clientGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="#7c3aed" />
            <stop offset="100%" stop-color="#6d28d9" />
          </linearGradient>

          <linearGradient id="netGrad" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stop-color="#0284c7" />
            <stop offset="50%" stop-color="#6366f1" />
            <stop offset="100%" stop-color="#a855f7" />
          </linearGradient>
        </defs>

        <!-- HOST MACHINE BOX -->
        <g class="machine-host" transform="translate(40, 40)">
          <rect width="280" height="280" rx="12" class="box-bg box-host-border" stroke-width="2" />
          
          <rect x="0" y="0" width="280" height="40" rx="12" fill="url(#hostGrad)" />
          <rect x="0" y="28" width="280" height="12" fill="url(#hostGrad)" />
          <text x="140" y="26" text-anchor="middle" fill="#ffffff" font-size="14" font-weight="700">HOST (Primary Workstation)</text>

          <!-- Host Subcomponents -->
          <!-- evdev / Physical Devices -->
          <g transform="translate(20, 55)">
            <rect width="240" height="42" rx="6" class="subbox-bg" stroke-width="1" />
            <text x="120" y="26" text-anchor="middle" class="subbox-text" font-size="11">Physical Devices (/dev/input/event*)</text>
          </g>

          <!-- nexus-kvmd Daemon -->
          <g transform="translate(20, 110)">
            <rect width="240" height="52" rx="6" class="subbox-bg" stroke="#0284c7" stroke-width="1.5" />
            <text x="120" y="24" text-anchor="middle" fill="#0284c7" font-size="12" font-weight="bold">nexus-kvmd (rkvm-server)</text>
            <text x="120" y="42" text-anchor="middle" class="subbox-subtext" font-size="10">TargetRouter &amp; Unix Socket 0660</text>
          </g>

          <!-- nexus-agent / Wayland Portal -->
          <g transform="translate(20, 175)">
            <rect width="240" height="45" rx="6" class="subbox-bg" stroke="#6366f1" stroke-width="1" />
            <text x="120" y="22" text-anchor="middle" fill="#6366f1" font-size="11" font-weight="600">nexus-agent (Wayland Portal)</text>
            <text x="120" y="37" text-anchor="middle" class="subbox-subtext" font-size="9.5">libei / reis • Pointer Barriers</text>
          </g>

          <!-- Tauri UI -->
          <g transform="translate(20, 230)">
            <rect width="240" height="36" rx="6" class="subbox-bg" stroke-width="1" />
            <text x="120" y="23" text-anchor="middle" class="subbox-text" font-size="11">NexusKVM GUI (Tauri 2 + React)</text>
          </g>
        </g>

        <!-- NETWORK CONNECTOR & PIPELINE -->
        <g class="network-pipe">
          <!-- TLS 1.3 Transport Stream 5258 -->
          <path d="M 320, 136 L 480, 136" stroke="url(#netGrad)" stroke-width="3.5" stroke-dasharray="6 4" class="anim-flow" />
          <circle cx="400" cy="136" r="16" class="box-bg" stroke="#0284c7" stroke-width="1.5" />
          <text x="400" y="140" text-anchor="middle" fill="#0284c7" font-size="9" font-weight="bold">TLS</text>

          <text x="400" y="115" text-anchor="middle" fill="#0284c7" font-size="10.5" font-weight="600">Port 5258/tcp</text>
          <text x="400" y="170" text-anchor="middle" class="subbox-subtext" font-size="9">rkvm-net Event Stream</text>

          <!-- Clipboard / Control Stream 5259 -->
          <path d="M 320, 200 L 480, 200" stroke="#6366f1" stroke-width="2" stroke-dasharray="3 3" class="anim-flow-reverse" />
          <text x="400" y="218" text-anchor="middle" fill="#6366f1" font-size="9.5">Port 5259/tcp (Clipboard/Control)</text>
        </g>

        <!-- CLIENT MACHINE BOX -->
        <g class="machine-client" transform="translate(480, 40)">
          <rect width="280" height="280" rx="12" class="box-bg box-client-border" stroke-width="2" />
          
          <rect x="0" y="0" width="280" height="40" rx="12" fill="url(#clientGrad)" />
          <rect x="0" y="28" width="280" height="12" fill="url(#clientGrad)" />
          <text x="140" y="26" text-anchor="middle" fill="#ffffff" font-size="14" font-weight="700">CLIENT (Target Machine)</text>

          <!-- Client Subcomponents -->
          <!-- rkvm-client -->
          <g transform="translate(20, 65)">
            <rect width="240" height="52" rx="6" class="subbox-bg" stroke="#a855f7" stroke-width="1.5" />
            <text x="120" y="24" text-anchor="middle" fill="#a855f7" font-size="12" font-weight="bold">rkvm-client (Client Runtime)</text>
            <text x="120" y="42" text-anchor="middle" class="subbox-subtext" font-size="10">TLS Decryption &amp; Verification</text>
          </g>

          <!-- /dev/uinput Driver -->
          <g transform="translate(20, 135)">
            <rect width="240" height="52" rx="6" class="subbox-bg" stroke="#c084fc" stroke-width="1" />
            <text x="120" y="24" text-anchor="middle" fill="#c084fc" font-size="11.5" font-weight="bold">/dev/uinput (Virtual Devices)</text>
            <text x="120" y="42" text-anchor="middle" class="subbox-subtext" font-size="9.5">Kernel Virtual Mouse &amp; Keyboard</text>
          </g>

          <!-- Desktop Compositor -->
          <g transform="translate(20, 205)">
            <rect width="240" height="55" rx="6" class="subbox-bg" stroke-width="1" />
            <text x="120" y="23" text-anchor="middle" class="subbox-text" font-size="11" font-weight="600">User Desktop Session / GDM</text>
            <text x="120" y="42" text-anchor="middle" class="subbox-subtext" font-size="9.5">GNOME (Wayland) / KDE / X11</text>
          </g>
        </g>
      </svg>
    </div>
  </div>
</template>

<script setup lang="ts">
// TopologyDiagram in English
</script>

<style scoped>
.topology-container {
  margin: 2rem 0;
  padding: 1.5rem;
  background: var(--vp-c-bg-card);
  border: 1px solid var(--vp-card-border);
  border-radius: 12px;
  backdrop-filter: blur(10px);
  box-shadow: var(--vp-card-shadow);
}

.topology-header {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 1.2rem;
}

.pulse-status {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--vp-c-brand-1);
  box-shadow: 0 0 10px var(--vp-c-brand-1);
  animation: pulseDot 2s infinite;
}

@keyframes pulseDot {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(1.3); }
}

.topology-title {
  font-weight: 700;
  font-size: 0.95rem;
  color: var(--vp-c-text-1);
  letter-spacing: 0.3px;
}

.topology-graphic-wrapper {
  width: 100%;
  overflow-x: auto;
}

.topology-svg {
  width: 100%;
  min-width: 600px;
  height: auto;
  display: block;
}

/* Theme Adaptive SVG Classes */
:root .box-bg {
  fill: #ffffff;
}
:root .box-host-border {
  stroke: #0284c7;
}
:root .box-client-border {
  stroke: #7c3aed;
}
:root .subbox-bg {
  fill: #f8fafc;
  stroke: #cbd5e1;
}
:root .subbox-text {
  fill: #1e293b;
}
:root .subbox-subtext {
  fill: #64748b;
}

.dark .box-bg {
  fill: #0f172a;
}
.dark .box-host-border {
  stroke: #38bdf8;
}
.dark .box-client-border {
  stroke: #c084fc;
}
.dark .subbox-bg {
  fill: #1e293b;
  stroke: #334155;
}
.dark .subbox-text {
  fill: #e2e8f0;
}
.dark .subbox-subtext {
  fill: #94a3b8;
}

.anim-flow {
  animation: flowLine 2s linear infinite;
}

.anim-flow-reverse {
  animation: flowLineRev 3s linear infinite;
}

@keyframes flowLine {
  to { stroke-dashoffset: -20; }
}

@keyframes flowLineRev {
  to { stroke-dashoffset: 20; }
}
</style>
