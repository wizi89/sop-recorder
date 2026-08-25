# Manual tests

Fixtures for the things unit tests cannot reach: behaviour that only exists once a
real OS is delivering real input events to a real recording.

## capture-step-count.html

Verifies that the input suppression rules turn physical input into the right
number of steps. Open it in a maximised browser, start a recording, work through
the four panels, stop, and count the PNGs.

```
ls "…/CogniClone Workflows/SOP <newest>/screenshots"/step_*.png | wc -l
```

Expected: **11** = 3 + 5 + 2 + 1.

| Panel | Do | Expect | Guards against |
|---|---|---|---|
| 1 | 3 double-clicks, same spot, mouse still | 3 steps from 6 clicks | the same-position window not collapsing a double-click |
| 2 | 5 different targets, as fast as you can | 5 steps | the old time-only debounce, which discarded distinct clicks under 300 ms apart |
| 3 | one click, then Enter immediately, mouse still | 2 steps | Enter inheriting the click's position and being suppressed as a repeat |
| 4 | hold Enter ~5 s | **1 step** | auto-repeat flooding the capture |

The page counts what it can see, so a wrong total tells you *where*:

- **Panel 2 shows your shortest gap in ms**, green under 250 ms. If your fastest
  gap was 400 ms you did not actually exercise the case; the suppression window
  is 250 ms and the point is to beat it at *different* positions.
- **Panel 3 shows the click-to-Enter interval**, green under 300 ms.
- **Panel 4 shows how many auto-repeat events Windows emitted.** That number is
  the test: ~80 repeats collapsing to one step is the guard working. Under four
  seconds of hold it turns red, because a short hold proves nothing.
- Clicks are counted on `document`, so a stray click on empty page area shows up
  too. A tally that missed those would quietly excuse a surplus step.

Press `r` to reset the counters. `r` is not a captured trigger, so it costs no step.

**Panel 4 is the one that matters.** Its failure mode is a lost recording rather
than a surplus step: enough uncollapsed repeats push the screenshot count past the
server's limit and the whole generation is refused. More than one or two steps
there is a stop-and-investigate, not a cosmetic miss.

### Reading the log alongside it

Every suppressed event is logged on purpose, with its position and the interval
since the previous capture, so the rule can be audited rather than trusted. A
release build writes to the usual application log directory; a debug build writes
to the path set in `src-tauri/src/lib.rs`.

Two traps when reading it, both of which have produced wrong conclusions before:

- **Check the log's first timestamp against the start of your recording.** Rotated
  entries are gone without a marker, and a truncated log reads exactly like a
  complete log of a shorter recording. Releases before 0.14.0 kept only 40 KB and
  discarded the overflow, so one held key could erase the minute before it.
- **Restart the recorder between comparison runs** on any build older than 0.14.0,
  where each recording installed another OS input hook and the N-th recording of a
  session therefore saw every click N times.
