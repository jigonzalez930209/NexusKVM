# Roadmap: switch por línea azul fiable y sin indicador de "pantalla compartida"

**Documento:** plan de trabajo correctivo
**Fecha:** Septiembre 2026
**Alcance:** Ubuntu + GNOME, NexusKVM host/client, switch de control al cruzar la línea azul (`EdgePortal`)
**Principio rector:** lo más simple siempre es lo mejor. Nada de portales, nada de adivinar peers, nada de estados nuevos.

---

## 0. Resumen ejecutivo

Dos síntomas persisten:

1. **S1 — Indicador "pantalla compartida" de Ubuntu/GNOME**: aparece al cruzar el borde. Causa conocida: cualquier sesión `InputCapture`/`RemoteDesktop`/`ScreenCast` del portal activa el indicador de *remote access* de Mutter. El agente NexusKVM ya no debe abrir esa sesión.
2. **S2 — El control no pasa de una PC a la otra** (o falla intermitentemente). Causa más probable hoy: **desincronización de binarios** entre UI, daemon, agente y servicios instalados. Al introducir `ControlCommand::Next`, una UI nueva contra un daemon viejo produce error de deserialización y el switch no ocurre. Un agente viejo (spawneado desde bundle o `PATH`) sigue abriendo el portal y mantiene S1.

El plan: diagnosticar el estado real de binarios, desplegar todo junto, verificar que el portal no existe, y validar el switch por borde como equivalente exacto del chord `Ctrl+Alt`.

---

## 1. Síntomas, objetivo y no-objetivos

### 1.1. Síntomas reportados

- [x] El indicador de pantalla compartida de Ubuntu se activa al pasar por la línea azul.
- [x] El control no cambia a la otra PC.
- [x] Cuando cambia, es poco fluido / falla seguido.

### 1.2. Objetivo

- [ ] Cruzar la línea azul = mismo efecto que `Left Alt + Left Ctrl` (ciclar al siguiente equipo conectado).
- [ ] Cero sesiones de portal InputCapture en host y client.
- [ ] Cambio de control en < 150 ms percibidos en LAN cableada.
- [ ] Fallo visible y recuperable: nunca quedar sin control local.
- [ ] Deploy reproducible en ambas PCs.

### 1.3. No-objetivos (evitar complicaciones)

- [ ] No reimplementar el switch con eventos sintéticos vía uinput (requiere privilegios y agrega carreras). `Next` ejecuta el mismo `switch_next()` del chord.
- [ ] No warp de cursor en esta iteración (el chord tampoco lo hace).
- [ ] No elegir peer en la UI (fuente de IDs obsoletos).
- [ ] No sumar estados nuevos al `Controller`.
- [ ] No reactivar el portal "por si acaso".

---

## 2. Estado actual del código (árbol sin commitear, relevante)

| Área | Cambio | Archivo |
|---|---|---|
| IPC | Nuevo `ControlCommand::Next` | `crates/nexus-common/src/ipc.rs` |
| Daemon | `Controller::next()` (sana + cicla conectados) | `crates/nexus-daemon/src/controller.rs` |
| Daemon | `InputTransport::next()` + `RkvmAdapter` | `crates/nexus-daemon/src/transport.rs` |
| Daemon | Rama IPC `Next` | `crates/nexus-daemon/src/ipc_server.rs` |
| rkvm | `Command::Next` cierra el chord (`finish_chord`) | `rkvm-master/rkvm-server/src/target.rs` |
| rkvm | Chord físico intacto, `switch_next()` | `rkvm-master/rkvm-server/src/server.rs` |
| UI | `switch_edge` host → `Next` (borrada elección de target) | `src-tauri/src/lib.rs` |
| Agente | Portal InputCapture eliminado de `main.rs` | `crates/nexus-agent/src/main.rs` |
| Docs | Nota de portal deshabilitado | `docs/WAYLAND_PORTAL.md` |

Pendiente de commit. **No commitear hasta cerrar Fase 1 y 3.**

---

## 3. Hipótesis ordenadas (diagnóstico)

| # | Hipótesis | Prob. | Evidencia | Cómo confirmar |
|---|---|---|---|---|
| H1 | UI nueva + daemon viejo: `Next` desconocido → error de parseo → no switch | Alta | `ipc_server` responde error si `serde_json` falla | Fase 0.3: `journalctl -u nexuskvm-host`; probar `ControlCommand::Next` con `nexusctl`/socket |
| H2 | Agente viejo corriendo (bundle `src-tauri/binaries` o `/usr/bin`) mantiene portal → S1 | Alta | `find_bin` cae a `PATH` si no hay binario nuevo | Fase 0.2: `pgrep -a nexus-agent`, `ls -l` fecha de binario |
| H3 | Daemon systemd viejo vs. daemon spawneado por la UI (dos sockets/versiones) | Media | `nexuskvm-host.service` + spawn de runtime | Fase 0.4: comparar PID y `--config` en uso |
| H4 | Peer desconectado / reconexión TLS: `switch_next` cicla y queda en local (no-op silencioso) | Media | Certificados y `AllowAnyAnonymousOrAuthenticatedClient` recientes | Fase 0.5: `nexusctl status`, logs de conexión `:5258` |
| H5 | Indicador causado por *Sharing* de Ubuntu ajeno a NexusKVM | Baja | `gnome-remote-desktop` activo | Fase 0.6: `grdctl status` / panel Compartir; matar agente y observar |
| H6 | Rebote/arming de `EdgePortal` (varios `switchEdge` por cruce) | Baja | debounce 300 ms en TS | Fase 0.7: log en UI + contador de comandos por cruce |

**Regla:** no tocar código hasta completar Fase 0. Cada hipótesis tiene comando de confirmación.

---

## 4. Fase 0 — Diagnóstico (bloqueante)

> Objetivo: saber exactamente qué binario corre en cada PC y quién abre el portal.

### 0.1. Inventario de binarios (ambas PCs)

```bash
ls -l --time-style=full-iso /usr/bin/nexus-kvmd /usr/bin/nexus-agent /usr/bin/nexusctl \
  /usr/libexec/nexuskvm/nexus-kvmd /usr/libexec/nexuskvm/nexus-agent 2>/dev/null
ls -l --time-style=full-iso ~/github/NexusKVM/target/release/nexus-kvmd \
  ~/github/NexusKVM/target/release/nexus-agent 2>/dev/null
ls -l --time-style=full-iso ~/github/NexusKVM/src-tauri/binaries/ 2>/dev/null
```

**Criterio:** todos los `mtime` deben ser posteriores al último cambio de source. Si `/usr/bin/nexus-agent` es viejo → H2 confirmada.

### 0.2. Procesos vivos

```bash
pgrep -a nexus-agent
pgrep -a nexus-kvmd
ps -o pid,lstart,cmd -p $(pgrep -x nexus-agent) 2>/dev/null
```

**Criterio:** un solo agente por sesión, con `--role host|client` y `--data-dir`. Si aparece sin argumentos o con fecha vieja → H2.

### 0.3. Daemon y protocolo

```bash
systemctl status nexuskvm-host --no-pager 2>/dev/null || systemctl status nexus-kvmd --no-pager
journalctl -u nexuskvm-host -n 120 --no-pager | grep -iE "next|unknown|parse|error|switch"
```

Prueba directa del comando nuevo (reemplazar socket/token si aplica):

```bash
sudo /usr/bin/nexusctl status --json   # si nexusctl existe; si no, usar la UI
# y luego: cruzar la línea azul una vez y ver si la respuesta ok=false con "unknown variant"
```

**Criterio:** el daemon debe aceptar `Next`. Si responde error de variante desconocida → H1 confirmada.

### 0.4. ¿Qué daemon usa la UI?

```bash
ls -l /proc/$(pgrep -f "nexus-kvmd" | head -1)/exe
systemctl show -p ExecStart nexuskvm-host --no-pager 2>/dev/null
```

**Criterio:** un único daemon; el mismo binario que se va a desplegar.

### 0.5. Estado de peers

```bash
sudo /usr/bin/nexusctl status --json 2>/dev/null
journalctl -u nexuskvm-host -n 200 --no-pager | grep -iE "Connected|Disconnected|peer|tls"
```

**Criterio:** peer `Connected` estable por > 60 s. Si reconecta en loop → H4; revisar certs/claves de `nexuskvm-enable-boot.sh`.

### 0.6. Origen real del indicador (S1)

```bash
pkill -x nexus-agent; sleep 3
# observar barra superior de Ubuntu
grdctl status 2>/dev/null || true
gsettings get org.gnome.desktop.remote-desktop.rdp enable 2>/dev/null || true
journalctl --user -u xdg-desktop-portal-gnome -n 80 --no-pager 2>/dev/null | grep -iE "inputcapture|session|enable"
```

**Criterio:** si el indicador desaparece al matar el agente → H2 (portal del agente viejo). Si sigue → H5 (Sharing de Ubuntu, no es NexusKVM).

### 0.7. Cruces por evento (opcional)

Agregar temporalmente `console.info('[edge] switchEdge', normalized)` en `EdgePortal.tsx` y contar cuántos disparos ocurren por cruce.

**Salida de Fase 0:** una tabla con H1..H6 confirmadas/descartadas y el plan de despliegue.

---

## 5. Fase 1 — Sincronizar y desplegar TODO junto

> Objetivo: eliminar H1/H2/H3 de una vez. UI, daemon, agente y cliente rkvm deben ser del mismo build.

### 1.1. Build único (PC host, o ambas)

```bash
cd ~/github/NexusKVM
scripts/build-runtime-bins.sh --release
```

Esto compila y stagea:

- `target/release/nexus-kvmd`, `nexus-agent`, `nexusctl`
- `rkvm-master/target/release/rkvm-client`
- copias en `src-tauri/binaries/*-<triple>` (sidecars que usa la UI)

### 1.2. Instalar en ambas PCs

```bash
sudo scripts/hotfix-edge-switch.sh
```

Instala `nexus-kvmd` (libexec + /usr/bin), `nexus-agent`, `nexusctl`, unit del host, y hace `systemctl restart nexuskvm-host` + `pkill -x nexus-agent`.

Si no se usa el script: instalar a mano y reiniciar servicios:

```bash
sudo install -Dm755 target/release/nexus-kvmd /usr/libexec/nexuskvm/nexus-kvmd
sudo install -Dm755 target/release/nexus-kvmd /usr/bin/nexus-kvmd
sudo install -Dm755 target/release/nexus-agent /usr/bin/nexus-agent
sudo systemctl restart nexuskvm-host.service
pkill -x nexus-agent || true
```

### 1.3. UI

- Dev: reiniciar `cargo tauri dev` (recompila `switch_edge` nuevo).
- Bundle: reconstruir la app para que los sidecars actualizados entren al paquete.

### 1.3.1. Obligatorio en el paquete (implementado)

- `scripts/nexuskvm-runtime-apply.sh` se instala en
  `/usr/libexec/nexuskvm/` y lo ejecuta el postinstall de **deb y rpm**.
- Hace: mirror a libexec, `pkill` de binarios viejos, `systemctl restart` de
  unidades habilitadas, y verificación de marcadores:
  - daemon: `ipc features: ipc-next`
  - agente: `edge-strip mode: InputCapture portal disabled`
- Falla el install (`exit 1`) si algo no verifica; escape:
  `NEXUSKVM_POSTINSTALL_STRICT=0`.
- `build-runtime-bins.sh` aborta si un binario falta o el sidecar staged no
  coincide (`cmp`).
- `hotfix-edge-switch.sh` ya no duplica lógica: llama al mismo apply script.

### 1.4. Verificación post-deploy

```bash
pgrep -a nexus-agent                 # debe decir "edge-strip mode: InputCapture portal disabled" en logs
journalctl --user -u nexus-agent -n 20 --no-pager 2>/dev/null | tail
# o si lo spawnea la UI:
tail -n 40 ~/.local/share/io.nexuskvm.app/logs/nexus-agent.log 2>/dev/null
ls -l /proc/$(pgrep -x nexus-agent | head -1)/exe
```

**Criterios:**

- [ ] Un solo agente, binario nuevo, log `edge-strip mode: InputCapture portal disabled`.
- [ ] Un solo daemon, binario nuevo.
- [ ] Indicador de pantalla compartida ausente en reposo.
- [ ] `nexusctl status` responde.

---

## 6. Fase 2 — Erradicar InputCapture (S1)

### 2.1. Ya hecho en source

- `crates/nexus-agent/src/main.rs` no importa `PortalBackend`/`EdgeEngine` ni registra sesión.
- Log de arranque para verificación.
- `docs/WAYLAND_PORTAL.md` documenta el motivo.

### 2.2. Tareas de cierre

- [ ] Añadir test de humo: el binario `nexus-agent` no contiene el símbolo de arranque de portal, o test unitario que verifique que `main` no llama `PortalBackend::connect` (por ejemplo `rg -q "PortalBackend::connect" crates/nexus-agent/src/main.rs` debe fallar).
- [ ] Decidir destino de `crates/nexus-agent/src/backend.rs` y `engine.rs`: mantener como referencia o borrar. Si se mantienen, marcar `#[allow(dead_code)]` no hace falta (son `pub`), pero evitar que alguien los use por accidente con un comentario de cabecera: "NO USAR: activa el indicador de pantalla compartida en GNOME".
- [ ] Correr `pkill -x nexus-agent` una vez en cada PC tras instalar, para matar el binario viejo.

### 2.3. Criterio de aceptación

- [ ] Cruzar el borde 20 veces: el indicador nunca aparece.
- [ ] `journalctl --user -u xdg-desktop-portal-gnome` no registra sesiones InputCapture durante las pruebas.

---

## 7. Fase 3 — Switch por borde = chord `Ctrl+Alt` (S2)

### 7.1. Diseño (ya implementado)

```
EdgePortal (línea azul) → api.switchEdge
  → Tauri switch_edge (host)  → ControlCommand::Next
      → Controller::next()  → transport.next()  → TargetHandle::switch_next()
          → TargetRouter::switch_next()  == mismo código que el chord físico
  → Tauri switch_edge (client) → PeerMessage::SwitchLocal al host
```

Ventajas:

- Sin elección de peer en la UI → sin IDs obsoletos.
- Solo cicla peers `Connected` (`TargetRouter::switch_next`).
- `PendingCommand::apply` drena teclas retenidas y emite releases (no más modificadores pegados).
- `finish_chord()` inmediato: los eventos nuevos van al destino nuevo.

### 7.2. Tareas de verificación

- [ ] Test unitario: `next_cycles_local_and_peer` (hecho, `controller.rs`).
- [ ] Test e2e IPC: enviar `ControlCommand::Next` y verificar `active_target` = peer, luego local. (Añadir a `crates/nexus-daemon/tests/e2e_ipc.rs`.)
- [ ] Test e2e target: `Command::Next` deja `chord_changed = false` (ya cubierto por `chord_keeps_release_on_previous_until_finished`; ampliar para el comando).
- [ ] Prueba manual: cruzar borde con un solo peer → va y vuelve; con dos peers → cicla en orden de conexión.

### 7.3. Compatibilidad de versiones (mitigación H1)

- [ ] Añadir fallback: si `Next` devuelve `ok=false` con error de parseo/variante, la UI usa `Switch` con el primer peer `Connected` (una sola vez, sin complejidad adicional). Documentar como puente temporal.
- [ ] Alternativa más simple y definitiva: versión de protocolo. Agregar campo `protocol: u32` a `ControlRequest`/`ControlResponse` y rechazar con mensaje claro "actualiza el daemon". Se decide al cerrar la fase.

### 7.4. Criterio de aceptación

- [ ] 50 cruces consecutivos host→client→host sin fallos ni teclas pegadas.
- [ ] El switch ocurre en < 150 ms desde el cruce.
- [ ] El atajo físico sigue funcionando.

---

## 8. Fase 4 — Fluidez y anti-rebote

### 8.1. UI (`EdgePortal.tsx`)

- [ ] Revisar `isArmedRef` / `leaveTimerRef`: un cruce = un `switchEdge`.
- [ ] Ajustar histéresis (`200 ms`) si el cursor queda sobre la franja de 8 px: no debe rearmar mientras el target no sea local.
- [ ] Evitar el segundo disparo al recibir `nexus-target-changed` con retraso (debounce actual 300 ms puede ser corto con IPC lento). Considerar bloquear hasta el evento de estado, no por tiempo.

### 8.2. Daemon

- [ ] `Controller::next()` ya sanea transiciones atascadas vía `sync_target()`.
- [ ] Confirmar que `ensure_local_transport()` corre antes de ciclar si el peer activo murió (ya en `sync_target`).
- [ ] Añadir timeout al `transport.next()` (p. ej. 1 s) para no bloquear el lock de transición si el server no responde.

### 8.3. Peer / red

- [ ] Loggear reconexiones con contador; si el peer reconecta más de 3 veces/min, revisar TLS (certs de `nexuskvm-enable-boot.sh`, `AllowAnyAnonymousOrAuthenticatedClient`).
- [ ] Verificar que `peer_id` estable por IP evita registros duplicados (`prune_clients`).

### 8.4. Criterio de aceptación

- [ ] Sin rebotes A→B→A en un cruce lento.
- [ ] Sin bloqueo del lock de transición (medir con logs de tiempo).

### 8.4.1. Supervisor y resync (implementado)

- [x] Supervisor en la UI (`runtime::spawn_supervisor`): cada 2 s verifica el
      proceso del rol (`nexus-kvmd`/`rkvm-client`) y el `nexus-agent`; si
      murieron, los relanza con backoff exponencial (2–30 s) y lo registra en
      `nexuskvm-ui.log`. `rkvm-client` salía al perder la conexión y quedaba
      muerto hasta reabrir la app a mano.
- [x] El daemon ahora hace `sync_target()` cada 2 s además de reaccionar a
      cambios del transporte: sana transiciones atascadas y peers muertos.
- [x] El server ya libera teclas y vuelve a local cuando el cliente se cae
      (`prune_and_release`, cada 250 ms).

### 8.5. Re-armado a prueba de eventos perdidos (implementado)

- [ ] `EdgePortal` re-arma por seguridad si sigue en local y pasaron > 1.5 s
      desde el último cruce, aunque el evento `nexus-target-changed` se haya
      perdido. Evita quedarse trabado tras el ciclo A→B→A.
- [ ] `Controller::next()` devuelve error si no hay peer conectado (antes
      quedaba en local en silencio), así la UI re-arma y se ve el fallo.
- [ ] Tests: `next_after_local_works_again` (unit) y
      `e2e_next_loops_local_peer_local_peer` (IPC).

---

## 9. Fase 5 — Observabilidad

- [ ] Log estructurado por switch: `edge switch: from=<target> to=<target> reason=strip duration_ms=N ok=<bool>`.
- [ ] Contador en `AppStatus` (opcional): `last_switch_ms`, `switch_failures`.
- [ ] `nexusctl`:
  - [ ] `nexusctl next` para probar el camino sin mouse.
  - [ ] `nexusctl doctor`: versiones de UI/daemon/agente, socket, peer, portal, permisos.
- [ ] Panel UI: mostrar backend activo ("edge strip") y último error de switch.
- [x] Rotación de logs en la UI: tope 8 MB por archivo (`.log.1` rotado) y
      64 MB por directorio; `read_log_tail` lee solo los últimos 256 KB.
      `nexus-agent.log` había llegado a 542 MB.
- [x] `switch_edge` registra inicio/fin/errores en `nexuskvm-ui.log`; el daemon
      registra `next`, `local` y peers conectados en `nexus-kvmd.log`.

---

## 10. Fase 6 — Pruebas

### 10.1. Automáticas

| Test | Crate | Estado |
|---|---|---|
| `next_cycles_local_and_peer` | nexus-daemon | Hecho |
| e2e IPC `Next` | nexus-daemon/tests | Pendiente |
| `Command::Next` cierra chord | rkvm-server | Pendiente |
| Chord físico intacto | rkvm-server | Hecho |
| `propagate-switch-keys=false` no traga Ctrl suelto | rkvm-server | Hecho (test manual/regresión) |

### 10.2. Manuales por PC

- [ ] Host → client con un peer.
- [ ] Vuelta client → host desde la franja del client.
- [ ] Dos peers: ciclo completo.
- [ ] Peer apagado: cruzar borde no cuelga; control local permanece.
- [ ] Desconectar red durante control remoto: retorno local automático.
- [ ] Suspender la PC remota y volver.
- [ ] Reiniciar daemon sin reiniciar la UI.
- [ ] Ctrl+C, Alt+Tab, mayúsculas: incólumes.

### 10.3. Matriz de regresión rápida (5 min)

```bash
cargo test -p nexus-daemon -p nexus-agent
(cd rkvm-master && cargo test -p rkvm-server)
scripts/test-all.sh    # si aplica
```

---

## 11. Criterios de aceptación globales

- [ ] El indicador de pantalla compartida nunca aparece por NexusKVM.
- [ ] El control pasa en ambos sentidos en 1 intento, 50/50 veces.
- [ ] El switch por borde y el chord `Ctrl+Alt` son indistinguibles en resultado.
- [ ] `Next` funciona con UI y daemon de la misma versión; el error de versión es claro.
- [ ] Deploy documentado y repetible en < 10 min por PC.
- [ ] Sin teclas/botones pegados tras 100 switches.
- [ ] Todos los tests automáticos pasan.

---

## 12. Riesgos y rollback

| Riesgo | Mitigación | Rollback |
|---|---|---|
| UI nueva + daemon viejo | Fase 3.3 (fallback/versión) | Reinstalar ambos; `pkill -x nexus-agent`; restaurar `Switch` en UI |
| Portal reaparece | Fase 2 (test de humo) | `pkill -x nexus-agent`; revisar binario con `ls -l /proc/PID/exe` |
| Rebote al cruzar | Fase 4 (arming por estado, no por tiempo) | Desactivar franja (`hide_edge_portal_cmd`) |
| Pérdida de control local | `nexusctl next`/`local` por SSH | `systemctl stop nexuskvm-host` |
| Falsos positivos de H5 | Fase 0.6 | No aplica (Sharing ajeno) |

---

## 13. Plan por días

| Día | Trabajo | Salida |
|---|---|---|
| D0 | Fase 0 completa en ambas PCs | Tabla H1..H6 |
| D1 | Fase 1 build+deploy+verificación | Indicador ausente, daemon nuevo |
| D2 | Fase 3 tests e2e + compat | `Next` verificado |
| D3 | Fase 4 fluidez + anti-rebote | 50 cruces limpios |
| D4 | Fase 5 observabilidad + `nexusctl doctor` | Diagnóstico sin terminal |
| D5 | Fase 6 matriz completa + commit | Release candidate |

---

## 14. Checklist de cierre

- [ ] `pgrep -a nexus-agent` muestra un único agente nuevo con log `edge-strip mode`.
- [ ] Portal InputCapture inexistente en host y client.
- [ ] `ControlCommand::Next` responde `ok=true` y cicla.
- [ ] 50 cruces limpios ida/vuelta.
- [ ] Atajo `Ctrl+Alt` intacto.
- [ ] Tests daemon/agent/rkvm verdes.
- [ ] Docs actualizadas (`WAYLAND_PORTAL.md`, este roadmap, troubleshooting).
- [ ] Commit atómico con mensaje claro.

---

## 15. Anexo: decisiones de simplicidad

1. **No inyectar Ctrl+Alt sintético** con uinput: mismo efecto que `Next`, con más permisos y carreras.
2. **No portal**: el indicador de GNOME no es suprimible por API.
3. **No elegir peer en la UI**: el router ya conoce los conectados.
4. **No warp**: el chord tampoco lo hace; se evalúa después si hace falta.
5. **Un solo comando**: `Next` es la única vía del borde en host; `SwitchLocal` en client.
