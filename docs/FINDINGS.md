# Findings

Hard-won, non-obvious facts about the recorder. Each entry cost real debugging time.
Read this before diagnosing anything in capture, window management, or permissions.

**Adding entries:** newest at the top. One entry per finding. Keep the shape
`Symptom / Cause / Rule`, because the symptom is what a future reader will search for.
Only add things that were genuinely surprising: if it is obvious from the code, skip it.

---

## 2026-09-04 A source-scanning test passed on macOS and failed on Windows, because git hands it CRLF

**Symptom:** `only_write_report_writes_a_report_file` failed in CI on Windows only, claiming three
report writers where the source plainly has one. `left: 3, right: 1`. The same commit passed
locally and every other test in the file passed on the same Windows run.

**Cause:** the guard reads its own source with `include_str!` and cuts the test half off with
`source.split("#[cfg(test)]\nmod ")`. `include_str!` embeds the file exactly as it sits on disk, and
git checks out CRLF on Windows -- so the needle containing a bare `\n` matched nothing, `.next()`
returned the *whole* file, and the count then included the two writes the test fixtures make on
purpose. The assertion message was about a second writer, which is the one thing that had not
happened.

A sibling guard in the same file survived only by luck: it searches for `"\n            \""`, and
`\r\n` happens to contain `\n`.

**Rule:** any test that reads source text must normalise first --
`include_str!(...).replace("\r\n", "\n")`. The failure cannot appear on a machine that checks out
LF, so it will only ever be found in CI, and it will look like the thing being guarded has broken
rather than the guard. Verified by converting the file to CRLF locally and watching the count go
back to 3.

**Related, found in the same run:** two tests in `breadcrumb_isolation` redirected the global
reports directory without taking `RING_TEST_LOCK`, which the four tests in `mod tests` do take. They
had raced happily for weeks. The lock now lives at file scope as `GLOBAL_STATE_TESTS` and every test
that touches a process global takes it. A lock inside one test module cannot protect a global that
another test module also writes.

---

## 2026-09-04 Tauri's unlisten is typed `() => void` but returns a promise that rejects

**Symptom:** the first live run of error reporting against staging produced an event nobody
triggered: `ui_error: undefined is not an object (evaluating 'listeners[eventId].handlerId')`,
phase `login`, seconds after an unrelated relaunch. `useErrorReports.ts` already carried a comment
about this exact message and a `try { stop?.() } catch {}` guard against it. The guard did not hold.

**Cause:** two things compound. Tauri 2.11's injected unlisten script
(`tauri-2.11.1/src/event/mod.rs:212`) reads `listeners[eventId].handlerId` after checking only that
the per-event object exists, never the entry. Its emit script twenty lines below checks both, so the
asymmetry is an oversight. The entry is missing whenever the webview's `window` is recreated while
Rust still holds listener ids -- an HMR reload under `tauri dev`, or a relaunch after a panic.

The second half is why the existing guard failed: `listen()` resolves to
`async () => _unlisten(event, eventId)`, so the failure arrives as a *rejected promise*, while the
declared type is `UnlistenFn = () => void`. A synchronous `try/catch` cannot see it, and because the
type hides the promise, no call site anywhere attaches a `.catch`. The rejection reaches
`unhandledrejection`, and with reporting on it files a report about our own teardown.

**Rule:** never invoke a Tauri unlisten function directly -- go through `safeUnlisten`, which
coerces the return through `Promise.resolve` and swallows both the throw and the rejection. A
source-scanning test in `safeUnlisten.test.ts` fails if a raw call reappears, because a raw call
looks correct to reviewers and to `tsc` alike. There were nine such sites across eight files; a
plain grep found eight of them.

---

## 2026-09-04 A report was written and no dialog opened, because only panics announced themselves

**Symptom:** Clicking a trigger in the settings window produced nothing. No dialog, no error, no log
line. The report file was on disk with correct content -- it simply never appeared.

**Cause:** `error_report:created` was emitted from exactly one place, the panic hook. `create()` --
the path behind every `ui_error` and `command_error` -- wrote the file and told nobody.

It looked correct for a year of reading because the only caller was the main window, whose
`useErrorReports.create` calls `refresh()` itself right after. Creator and dialog were the same
window, so the missing event was invisible. The moment a report was raised from a *different* window
(the settings window is a window of its own), the main window had nothing to react to and only found
the report on its next mount.

`Emitter::emit` reaches every webview, so one emit is all this ever needed.

**Rule:** when a side effect must accompany a state change, put it inside the function that makes the
change rather than beside each call. The announcement now lives in `write_report`, the single
function that puts a report on disk, and a test asserts nothing else in the shipping half of the
file writes a report file -- verified by adding a second writer and watching it fail. "Every caller
remembers to announce" is not a property anything can check; "there is one writer" is.

---

## 2026-09-04 Three of the eight report phases were produced by nobody

**Symptom:** A panic triggered from the settings window arrived tagged `phase: idle`. Reading further,
every error the webview raised was tagged `unknown`, whatever the user had been doing.

**Cause:** `phase` was set from six scattered `set_phase` calls in Rust plus two hardcoded `"unknown"`
strings in the webview. Between them they could produce `startup`, `idle`, `recording` and
`processing` -- and nothing else. `Login`, `Review` and `Settings` were declared in the enum,
serialised by serde, listed in the design and accepted by the server's `PHASES` literal, so
everything looked complete from any single file. Only Rust set the phase, and `login`, `review` and
`settings` are screens the webview knows about and Rust cannot see.

Nothing failed. A wrong phase is still a valid phase, so no test, type or schema had anything to
object to, and the tag that answers "what was the user doing" answered "idle" or "unknown" almost
every time. `phase` is also part of the fingerprint for everything except panics
(`[kind, phase, normalised message]`), so failures on unrelated screens grouped together.

**Rule:** when a value is an enum whose variants are produced by scattered assignments, put the
derivation in one pure function and assert its *range* covers the enum -- `phaseForScreen` in
`src/lib/errorPhase.ts`, with a test that fails if a variant becomes unreachable and another that
reads the Rust enum so the two lists cannot drift. A value that only a human reading a report would
notice is wrong needs a test that looks at the code, not only at behaviour.

---

## 2026-09-04 The error reporter reported itself, and the dialog froze the app

**Symptom:** Clicking "Bericht senden" left the app apparently hung -- both buttons dead behind a
full-screen backdrop, no way out but quitting. On disk sat three error reports, all carrying the
same message: `undefined is not an object (evaluating 'listeners[eventId].handlerId')`. Two of them
were written 22 microseconds apart.

**Cause:** three faults stacked, and only the last one was visible.

`useErrorReports` registered its `error_report:created` listener without the `cancelled` guard that
`useSSE` already uses. Under React StrictMode the effect mounts, unmounts and mounts again, so the
cleanup ran before `listen` had resolved and unregistered a listener that did not exist yet. Tauri
threw out of an unawaited promise.

The global `unhandledrejection` handler in `main.tsx` then filed a `ui_error` report about that
throw, which emitted `error_report:created`, which opened the consent dialog. The reporting
machinery was reporting itself, once per StrictMode mount -- hence the microsecond-apart pair.

The freeze was separate. `current` is `pending[0]`, so answering one report does not unmount the
dialog; the next report slides into the same mounted component. `busy`, set on click and cleared
only by unmount, therefore stayed `true`, and both buttons stayed disabled forever.

**Rule:** every `listen` cleanup needs the `cancelled` guard and a `try` around the unlisten --
tearing down twice must not throw. A component whose identity outlives its subject must reset its
per-subject state on that subject's id, not rely on unmounting. And an error handler that can be
triggered by its own failure needs a dedup window, or one fault becomes a queue of dialogs.

---

## 2026-08-29 xcap reports monitor geometry in different units per platform, and nothing says so

**Symptom:** On a single-monitor Windows laptop at 150% display scaling, every step logged

```
Rescaling monitor capture from 2560x1440 to 3840x2160 for the shared canvas
Resizing screenshot from 3840x2160 to 1920x1080
```

The first line should never appear on a single-monitor desktop, and the code comment directly above
it says as much: the resample "is skipped on the overwhelmingly common single-monitor desktop rather
than paying for a no-op resample". Screenshots still came out at the right size with the click
marker in the right place, so nothing looked broken. What it cost was sharpness (an upscale followed
by a downscale is softer than the single downscale it replaced), three times the canvas memory, and
about a second of wall clock on every captured step. At 100% scaling the scale factor is 1.0 and the
whole thing disappears, so a developer machine at 100% sees nothing at all.

**Cause:** `capture_full_screen` sized each monitor's slot on the shared canvas as
`Monitor::width() * max_scale_factor`. `xcap` does not report `width()` in the same units on every
platform, and its API gives no hint of this:

| Platform | `Monitor::width()` | Source |
|---|---|---|
| macOS | logical points | `CGDisplayBounds` |
| Windows | physical pixels | `dmPelsWidth` (`EnumDisplaySettings`) |

On macOS the multiplication is correct and necessary: 1512 points x 2.0 = 3024 pixels, which is
exactly what `capture_image` returns, so the resize is skipped. On Windows the geometry is *already*
physical, so multiplying by 1.5 asked for 3839x2160 from a capture that was already 2560x1440. The
mixed-DPI canvas logic was written and tested on a Mac, where it is right, and the same expression
is wrong on Windows for a reason no type or name exposes. Note the target was 3839, not 3840: the
reported scale factor is 1.4997071, so the old path also forced a non-integer resample.

The misleading part is that the result stays *geometrically* correct. Monitor origins, the cursor
position, and the click marker are all scaled by the same factor and the final resize to 1920x1080
normalises everything, so the marker lands within a pixel of where it should. Only the log line and
the step latency give it away.

**Rule:** never combine `Monitor::scale_factor()` with `Monitor::width()/height()/x()/y()` and
assume you know what space the result is in. Derive the density from the captures instead:
`capture_image().width() / Monitor::width()` is the pixels-per-geometry-unit ratio, it needs no
per-platform branch, and it collapses to 1.0 on Windows and 2.0 on a Retina Mac. That is what
`canvas_scale` in `capture/screenshot.rs` now does, guarded by tests that assert a single-monitor
desktop needs no resample on either platform. More generally: this module has two coordinate spaces
that coincide on Windows and do not on macOS, so a change that is verified on one platform is not
verified. The log line is the cheap check. If "Rescaling monitor capture" appears on a
single-monitor machine, the units are wrong again.
