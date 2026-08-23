<template>
  <div class="rkvm-citation-card">
    <div class="citation-header">
      <div class="citation-badge">
        <svg class="badge-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
        </svg>
        <span>Attribution &amp; Citation to rkvm</span>
      </div>
      <span class="license-tag">MIT License</span>
    </div>

    <div class="citation-body">
      <p class="citation-desc">
        <strong>NexusKVM</strong> is built upon an advanced fork of 
        <a href="https://github.com/htrefil/rkvm" target="_blank" rel="noopener noreferrer" class="rkvm-link">
          <strong>rkvm 0.6.1</strong>
        </a>, originally created by <strong>Florian Larysch</strong>.
      </p>

      <blockquote class="citation-quote">
        "rkvm is a tool for sharing keyboard and mouse between multiple Linux machines over the local network."
        <footer class="quote-author">— Florian Larysch (htrefil) &amp; rkvm contributors</footer>
      </blockquote>

      <div class="citation-grid">
        <div class="citation-item">
          <span class="item-title">📦 Core Network Protocol</span>
          <p>We preserve full compatibility with the high-performance binary event stream (<code>rkvm-net</code>).</p>
        </div>
        <div class="citation-item">
          <span class="item-title">⚡ NexusKVM Evolution</span>
          <p>Transformed into a dual lib + bin crate, adding asynchronous <code>TargetRouter</code>/<code>TargetHandle</code>, stable <code>SocketAddr</code> routing, and automatic fail-safe to <code>local</code>.</p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// RkvmCitation component in English
</script>

<style scoped>
.rkvm-citation-card {
  margin: 1.8rem 0;
  padding: 1.5rem;
  background: var(--vp-c-bg-card);
  border: 1px solid var(--vp-card-border);
  border-radius: 12px;
  box-shadow: var(--vp-card-shadow);
  backdrop-filter: blur(12px);
  position: relative;
  overflow: hidden;
}

.rkvm-citation-card::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 3px;
  background: linear-gradient(90deg, #0284c7, #818cf8, #c084fc);
}

.citation-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.citation-badge {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 700;
  font-size: 0.95rem;
  color: var(--vp-c-brand-1);
  letter-spacing: 0.3px;
}

.badge-icon {
  width: 18px;
  height: 18px;
}

.license-tag {
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
  border: 1px solid var(--vp-card-border);
  padding: 0.2rem 0.6rem;
  border-radius: 9999px;
  font-size: 0.75rem;
  font-weight: 600;
}

.citation-desc {
  color: var(--vp-c-text-1);
  font-size: 0.95rem;
  line-height: 1.6;
  margin-bottom: 1rem;
}

.rkvm-link {
  color: var(--vp-c-brand-1);
  text-decoration: underline;
  text-underline-offset: 3px;
}

.citation-quote {
  margin: 0.8rem 0 1.2rem 0;
  padding: 0.8rem 1.2rem;
  background: var(--vp-c-bg-soft);
  border-left: 3px solid var(--vp-c-brand-2);
  border-radius: 0 8px 8px 0;
  font-style: italic;
  color: var(--vp-c-text-2);
  font-size: 0.9rem;
}

.quote-author {
  display: block;
  margin-top: 0.4rem;
  font-style: normal;
  font-size: 0.8rem;
  color: var(--vp-c-text-3);
  text-align: right;
}

.citation-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
}

@media (max-width: 640px) {
  .citation-grid {
    grid-template-columns: 1fr;
  }
}

.citation-item {
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  padding: 0.8rem 1rem;
}

.item-title {
  display: block;
  font-weight: 600;
  font-size: 0.85rem;
  color: var(--vp-c-brand-1);
  margin-bottom: 0.3rem;
}

.citation-item p {
  margin: 0;
  font-size: 0.82rem;
  color: var(--vp-c-text-2);
  line-height: 1.45;
}
</style>
