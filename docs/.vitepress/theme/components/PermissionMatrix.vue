<template>
  <div class="perm-matrix-container">
    <div class="perm-table-wrapper">
      <table class="perm-table">
        <thead>
          <tr>
            <th>Component / Process</th>
            <th>System Resource</th>
            <th>Permissions / Group</th>
            <th>Security Mechanism</th>
            <th>Privilege Level</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td><strong>nexus-kvmd (Host)</strong></td>
            <td><code>/dev/input/event*</code></td>
            <td><code>input</code> group (0660)</td>
            <td>udev rule + <code>uaccess</code> tag</td>
            <td><span class="badge badge-system">System Service / User</span></td>
          </tr>
          <tr>
            <td><strong>rkvm-client (Client)</strong></td>
            <td><code>/dev/uinput</code></td>
            <td><code>input</code> group (0660)</td>
            <td>Kernel <code>uinput</code> module + udev</td>
            <td><span class="badge badge-system">System Service / User</span></td>
          </tr>
          <tr>
            <td><strong>nexus-agent (Host)</strong></td>
            <td>Wayland Session D-Bus</td>
            <td>Active user session</td>
            <td><code>org.freedesktop.portal.InputCapture</code></td>
            <td><span class="badge badge-user">Session User</span></td>
          </tr>
          <tr>
            <td><strong>NexusKVM GUI (Tauri)</strong></td>
            <td>Local Unix Socket</td>
            <td><code>0660</code> (matching UID/GID)</td>
            <td>Local IPC without root elevation</td>
            <td><span class="badge badge-unprivileged">Unprivileged</span></td>
          </tr>
          <tr>
            <td><strong>nexusctl (CLI)</strong></td>
            <td><code>/run/nexuskvm.sock</code></td>
            <td>Read/Write 0660</td>
            <td>Access via group membership</td>
            <td><span class="badge badge-user">User / Operator</span></td>
          </tr>
        </tbody>
      </table>
    </div>

    <div class="security-alert-box">
      <div class="alert-icon">⚠️</div>
      <div class="alert-content">
        <strong>Golden Rule of Security:</strong>
        <p>Never execute <code>chmod 666 /dev/uinput</code> or <code>chmod 666 /dev/input/event*</code>. Granting universal read/write permissions enables any unprivileged background process, browser tab exploit, or script to intercept all keystrokes (<em>keylogging</em>) or inject arbitrary input. Always use group membership (<code>input</code> group) and dynamic udev session tags (<code>TAG+="uaccess"</code>).</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// PermissionMatrix component in English
</script>

<style scoped>
.perm-matrix-container {
  margin: 1.5rem 0;
}

.perm-table-wrapper {
  overflow-x: auto;
  border-radius: 8px;
  border: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg-card);
}

.perm-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.88rem;
  text-align: left;
}

.perm-table th {
  background: var(--vp-c-bg-soft);
  color: var(--vp-c-text-1);
  padding: 0.75rem 1rem;
  font-weight: 600;
  border-bottom: 2px solid var(--vp-c-divider);
}

.perm-table td {
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--vp-c-divider);
  color: var(--vp-c-text-2);
}

.perm-table tr:hover td {
  background: var(--vp-c-brand-soft);
}

.badge {
  display: inline-block;
  padding: 0.2rem 0.5rem;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 600;
}

.badge-system {
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
  border: 1px solid var(--vp-card-border);
}

.badge-user {
  background: rgba(99, 102, 241, 0.15);
  color: #6366f1;
  border: 1px solid rgba(99, 102, 241, 0.3);
}

.badge-unprivileged {
  background: rgba(16, 185, 129, 0.15);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.3);
}

.security-alert-box {
  margin-top: 1rem;
  display: flex;
  gap: 0.8rem;
  background: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.25);
  border-radius: 8px;
  padding: 1rem;
}

.alert-icon {
  font-size: 1.3rem;
  line-height: 1;
}

.alert-content strong {
  color: #ef4444;
  display: block;
  margin-bottom: 0.3rem;
}

.alert-content p {
  margin: 0;
  font-size: 0.85rem;
  color: var(--vp-c-text-2);
  line-height: 1.5;
}
</style>
