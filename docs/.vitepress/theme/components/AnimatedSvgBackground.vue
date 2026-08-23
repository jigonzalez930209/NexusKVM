<template>
  <div class="animated-svg-bg-container" aria-hidden="true">
    <svg class="animated-svg-canvas" xmlns="http://www.w3.org/2000/svg">
      <defs>
        <!-- Gradients -->
        <linearGradient id="stream-cyan" x1="0%" y1="0%" x2="100%" y2="0%">
          <stop offset="0%" stop-color="#00d2ff" stop-opacity="0" />
          <stop offset="50%" stop-color="#0ea5e9" stop-opacity="0.8" />
          <stop offset="100%" stop-color="#00d2ff" stop-opacity="0" />
        </linearGradient>

        <linearGradient id="stream-purple" x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stop-color="#ec4899" stop-opacity="0" />
          <stop offset="50%" stop-color="#8b5cf6" stop-opacity="0.8" />
          <stop offset="100%" stop-color="#06b6d4" stop-opacity="0" />
        </linearGradient>

        <!-- Radial Glow Filters -->
        <filter id="nexus-glow" x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="5" result="blur1" />
          <feMerge>
            <feMergeNode in="blur1" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>

        <radialGradient id="mesh-glow-1" cx="20%" cy="30%" r="40%">
          <stop offset="0%" stop-color="#0ea5e9" class="glow-stop-1" />
          <stop offset="100%" stop-color="#0ea5e9" stop-opacity="0" />
        </radialGradient>

        <radialGradient id="mesh-glow-2" cx="80%" cy="70%" r="50%">
          <stop offset="0%" stop-color="#a855f7" class="glow-stop-2" />
          <stop offset="100%" stop-color="#a855f7" stop-opacity="0" />
        </radialGradient>

        <!-- Pattern Grid -->
        <pattern id="kvm-cyber-grid" width="60" height="60" patternUnits="userSpaceOnUse">
          <circle cx="30" cy="30" r="1" class="grid-dot" />
          <path d="M 60 0 L 0 0 0 60" fill="none" class="grid-line" />
        </pattern>
      </defs>

      <!-- Ambient Glow Layers -->
      <rect width="100%" height="100%" fill="url(#mesh-glow-1)" />
      <rect width="100%" height="100%" fill="url(#mesh-glow-2)" />

      <!-- Isometric / Cyber Grid -->
      <rect width="100%" height="100%" fill="url(#kvm-cyber-grid)" class="cyber-grid-rect" />

      <!-- Constellation / KVM Network Nodes & Connecting Wave Beams -->
      <!-- Path 1: Host to Client A -->
      <path
        d="M -100,180 Q 300,80 700,240 T 1500,120 T 2200,300"
        fill="none"
        class="wave-line wave-1"
      />
      <path
        d="M -100,180 Q 300,80 700,240 T 1500,120 T 2200,300"
        fill="none"
        class="pulse-beam pulse-beam-1"
      />

      <!-- Path 2: Client B to Nexus Relay -->
      <path
        d="M -100,450 C 400,250 800,650 1300,380 S 1900,550 2300,420"
        fill="none"
        class="wave-line wave-2"
      />
      <path
        d="M -100,450 C 400,250 800,650 1300,380 S 1900,550 2300,420"
        fill="none"
        class="pulse-beam pulse-beam-2"
      />

      <!-- Path 3: Fast Transit Channel -->
      <path
        d="M 100,850 C 500,750 900,900 1400,700 S 1950,850 2400,650"
        fill="none"
        class="wave-line wave-3"
      />
      <path
        d="M 100,850 C 500,750 900,900 1400,700 S 1950,850 2400,650"
        fill="none"
        class="pulse-beam pulse-beam-3"
      />

      <!-- Floating Nexus Circuit Nodes -->
      <g class="nodes-group">
        <!-- Node 1: Host Core -->
        <g class="node-item node-item-1" transform="translate(280, 115)">
          <circle r="16" class="node-ring" />
          <circle r="6" class="node-core cyan" filter="url(#nexus-glow)" />
          <circle r="2" fill="#ffffff" />
        </g>

        <!-- Node 2: Wayland InputCapture Gateway -->
        <g class="node-item node-item-2" transform="translate(680, 235)">
          <circle r="22" class="node-ring dashed" />
          <circle r="8" class="node-core indigo" filter="url(#nexus-glow)" />
          <circle r="3" fill="#ffffff" />
        </g>

        <!-- Node 3: rkvm TargetHandle Router -->
        <g class="node-item node-item-3" transform="translate(1180, 150)">
          <circle r="18" class="node-ring" />
          <circle r="7" class="node-core purple" filter="url(#nexus-glow)" />
          <circle r="2.5" fill="#ffffff" />
        </g>

        <!-- Node 4: Client Display Matrix -->
        <g class="node-item node-item-4" transform="translate(420, 390)">
          <circle r="14" class="node-ring" />
          <circle r="5" class="node-core cyan" filter="url(#nexus-glow)" />
        </g>

        <!-- Node 5: TLS 1.3 Secure Enclave -->
        <g class="node-item node-item-5" transform="translate(950, 480)">
          <circle r="20" class="node-ring dashed" />
          <circle r="7" class="node-core purple" filter="url(#nexus-glow)" />
          <circle r="2.5" fill="#ffffff" />
        </g>

        <!-- Node 6: Edge Portal Barrier Listener -->
        <g class="node-item node-item-6" transform="translate(1420, 360)">
          <circle r="16" class="node-ring" />
          <circle r="6" class="node-core indigo" filter="url(#nexus-glow)" />
        </g>

        <!-- Node 7: uinput Virtual Device Dispatcher -->
        <g class="node-item node-item-7" transform="translate(750, 780)">
          <circle r="24" class="node-ring" />
          <circle r="8" class="node-core cyan" filter="url(#nexus-glow)" />
          <circle r="3" fill="#ffffff" />
        </g>
      </g>
    </svg>
  </div>
</template>

<script setup lang="ts">
// Animated SVG Background Component
</script>

<style scoped>
.animated-svg-bg-container {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  pointer-events: none;
  z-index: 0;
  overflow: hidden;
}

.animated-svg-canvas {
  width: 100%;
  height: 100%;
}

/* Light / Dark Mode dynamic opacity */
:root .glow-stop-1 {
  stop-opacity: 0.08;
}
:root .glow-stop-2 {
  stop-opacity: 0.06;
}
:root .cyber-grid-rect {
  opacity: 0.35;
}
:root .grid-dot {
  fill: #0284c7;
  opacity: 0.15;
}
:root .grid-line {
  stroke: rgba(2, 132, 199, 0.05);
  stroke-width: 0.8;
}
:root .wave-line {
  opacity: 0.2;
}

.dark .glow-stop-1 {
  stop-opacity: 0.16;
}
.dark .glow-stop-2 {
  stop-opacity: 0.12;
}
.dark .cyber-grid-rect {
  opacity: 0.65;
}
.dark .grid-dot {
  fill: #38bdf8;
  opacity: 0.22;
}
.dark .grid-line {
  stroke: rgba(56, 189, 248, 0.05);
  stroke-width: 0.8;
}
.dark .wave-line {
  opacity: 0.28;
}

.wave-line {
  stroke-width: 1.5;
}

.wave-1 {
  stroke: #0ea5e9;
}

.wave-2 {
  stroke: #818cf8;
}

.wave-3 {
  stroke: #c084fc;
}

/* Pulsing Data Beams */
.pulse-beam {
  stroke-width: 3.5;
  stroke-linecap: round;
  stroke-dasharray: 80 400;
  filter: drop-shadow(0 0 6px rgba(14, 165, 233, 0.6));
  animation: dataStream 10s linear infinite;
}

.pulse-beam-1 {
  stroke: url(#stream-cyan);
  animation-duration: 7s;
}

.pulse-beam-2 {
  stroke: url(#stream-purple);
  animation-duration: 11s;
  animation-delay: -3s;
}

.pulse-beam-3 {
  stroke: url(#stream-cyan);
  animation-duration: 9s;
  animation-delay: -5s;
}

@keyframes dataStream {
  0% {
    stroke-dashoffset: 2000;
  }
  100% {
    stroke-dashoffset: 0;
  }
}

/* Node Elements */
.node-ring {
  fill: none;
  stroke: var(--vp-c-text-3);
  stroke-width: 1.2;
  opacity: 0.3;
  animation: pulseRing 4s ease-in-out infinite alternate;
}

.node-ring.dashed {
  stroke-dasharray: 4 3;
  animation: rotateRing 20s linear infinite;
  transform-origin: center;
}

.node-core.cyan {
  fill: #0ea5e9;
}

.node-core.indigo {
  fill: #6366f1;
}

.node-core.purple {
  fill: #a855f7;
}

.node-item-1 { animation: floatNode 6s ease-in-out infinite alternate; }
.node-item-2 { animation: floatNode 8s ease-in-out infinite alternate 1s; }
.node-item-3 { animation: floatNode 7s ease-in-out infinite alternate 2s; }
.node-item-4 { animation: floatNode 5s ease-in-out infinite alternate 0.5s; }
.node-item-5 { animation: floatNode 9s ease-in-out infinite alternate 1.5s; }
.node-item-6 { animation: floatNode 7.5s ease-in-out infinite alternate 3s; }
.node-item-7 { animation: floatNode 6.5s ease-in-out infinite alternate 2.5s; }

@keyframes floatNode {
  0% { transform: scale(0.98); }
  100% { transform: translateY(10px) scale(1.02); }
}

@keyframes pulseRing {
  0% { opacity: 0.2; stroke-width: 1; }
  100% { opacity: 0.55; stroke-width: 1.6; }
}

@keyframes rotateRing {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
