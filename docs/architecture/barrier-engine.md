# Barrier Engine & Hysteresis

The NexusKVM barrier engine manages smooth, natural mouse cursor transitions between physically separate displays and workstations.

---

## 1. Multi-Screen Spatial Challenges

Sharing a single mouse across physical computers introduces three primary challenges:

1. **Resolution & DPI Scaling Discrepancies:** A workstation might feature a 4K display (3840x2160) at 150% scaling, while the adjacent laptop features a 1080p display (1920x1080) at 100%.
2. **Edge Oscillation (*Jitter / Ping-Pong*):** Without hysteresis control, rapid mouse movement along a boundary can cause the cursor to bounce uncontrollably between screens.
3. **Partial Boundaries:** Screens of different physical heights placed side-by-side where only a specific subsection of the border should permit transition.

---

## 2. Normalized Proportional Coordinate Mapping

NexusKVM uses a mathematical model based on **normalized relative coordinates** $[0.0, 1.0]$:

$$\text{Normalized Position } y_{\text{norm}} = \frac{y_{\text{host}} - y_{\text{edge\_start}}}{y_{\text{edge\_end}} - y_{\text{edge\_start}}}$$

$$\text{Target Entry Coordinate } y_{\text{client}} = y_{\text{client\_start}} + y_{\text{norm}} \times (y_{\text{client\_end}} - y_{\text{client\_start}})$$

```
  +----------------------+
  | HOST (4K - 3840x2160)|
  |                      | -> [Right Edge: y = 1080 (50%)]
  |          x           |               |
  |                      |               v
  +----------------------+    +--------------------+
                              | CLIENT (1080p)     |
                              |                    |
                              | -> [Left Entry:    |
                              |     y = 540 (50%)] |
                              +--------------------+
```

Through this proportional transformation, the cursor appears on the remote display at the **exact matching relative visual height**, creating the sensation of one contiguous physical monitor.

---

## 3. Hysteresis & Anti-Jitter Safeguards

To ensure transitions feel intentional:
- **Pressure Threshold:** The cursor must sustain directional movement against the barrier for a configurable window (15–30 ms).
- **Inward Landing Zone:** Upon crossing to the receiving machine, the virtual cursor is placed several pixels inward from the border to prevent instant accidental bounce-back.
- **Partial Zones in `layout.json`:** Using `start_percent` and `end_percent`, you can restrict crossing exclusively to the physical overlap area between your monitors.
