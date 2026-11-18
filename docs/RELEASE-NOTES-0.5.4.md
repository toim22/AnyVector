# AnyVector 0.5.4

AnyVector 0.5.4 fixes a freeze when tracing bitmaps.

## Fixes

- **Tracing no longer freezes.** Quick Trace and PowerTRACE could stop responding on images with soft, anti-aliased
  edges, such as JPEG logos, whenever more than two colors were traced. They now finish in well under a second on
  images like the one reported. The same fix also covers freehand drawing and Reduce Nodes, which fit curves the
  same way.
