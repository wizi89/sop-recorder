# Server contract

The wire format between the recorder and its server: the endpoints, fields and SSE events this app
uses, as the client sees them. Both sides follow this file. It describes the wire only, not how the
server is built.

## Base URLs

From `src-tauri/src/config.rs:1-6`. The upload target setting picks the environment; a runtime
config `api_url` overrides the API base.

| Environment | API (`API_URL_*`)                 | Web app (`WEBAPP_URL_*`)         |
|-------------|-----------------------------------|----------------------------------|
| Production  | `https://api.wizimate.com`        | `https://app.cogniclone.ai`      |
| Staging     | `https://api.staging.cogniclone.ai` | `https://staging.cogniclone.ai` |
| Local       | `http://localhost:8000`           | `http://localhost:3000`          |

## Conventions

- JSON requests send `Content-Type: application/json`.
- "Bearer" means `Authorization: Bearer <access_token>`.
- Refusals the server authors carry `{"error": <code>, "message": <German text>}`; the client shows
  `message` when present.

## API endpoints

### `POST /auth/signin`

Source: `src-tauri/src/network/auth.rs:83` (`sign_in`). Auth: none.

- Request: `{"email": string, "password": string}`.
- Response fields read: `access_token`, `refresh_token`, `email`, `message`, `error` (all optional).
- Status codes: none branched on. A body without `access_token` is a failed sign-in; the client
  shows `message`, else `error`, else a generic text.

### `POST /auth/desktop/exchange`

Source: `src-tauri/src/network/auth.rs:285` (`complete_within`) and `auth.rs:357`
(`probe_exchange_within`). Auth: none.

- Request: `{"code": string, "code_verifier": string}`. `code` comes from the browser callback; the
  verifier is the PKCE verifier whose S256 challenge went to `/desktop/authorize`. Neither the
  challenge nor the `state` is sent.
- Response fields read: same as `/auth/signin`.
- Status codes: 404 or 405, or a body that is not JSON of that shape, means the server lacks browser
  sign-in. Any other answer counts as supported; no `access_token` is a failed sign-in.
- Probe: before opening the browser the client posts the body `{}` (5 s timeout) and applies the same
  rule. The release workflow's `server-contract` job uses it too.
- Timeout for the real exchange: 15 s.

### `POST /auth/refresh`

Source: `src-tauri/src/network/auth.rs:408` (`refresh_session`). Auth: none.

- Request: `{"refresh_token": string}`.
- Response fields read: `access_token`, `refresh_token`, `email` (all optional). A new
  `refresh_token` replaces the stored one.
- Status codes: 400, 401, 403 mean the token is rejected and the user is signed out. Any other
  non-2xx is a transient error. A 2xx without `access_token` also signs the user out.

### `POST /generate`

Source: `src-tauri/src/network/upload.rs:28` (`upload_multipart`). Auth: bearer.
`multipart/form-data`, 300 s timeout.

Parts:

- `audio`: file `recording.wav`, `audio/wav`.
- `step_NN` (two digits, one per screenshot, `NN` is the step order): `image/png`, file name from the
  local screenshot. `NN` may skip numbers (a failed capture, or a step deleted on the review screen);
  the parts are paired with `metadata.steps` in ascending `NN`.
- `metadata`: JSON text with
  - `guide_title`: string
  - `step_count`: number of screenshots
  - `pipeline_version`: integer
  - `generation_model`: string
  - `steps` (optional, only when there is one entry per screenshot): array of
    `{order, timestamp_seconds, click_x, click_y, trigger, marker_box?, capture_scope, password_check}`;
    `click_x`/`click_y` may be null, `marker_box` is `{x0, y0, x1, y1}` and is left out when absent,
    `capture_scope` is `window`, `monitor`, `monitor_fallback` or `unknown`, `password_check` is
    `ok`, `flagged`, `off` or `unknown` (`src-tauri/src/output/step_meta.rs`).
  - `capture_scopes`: object counting the recording's screenshots per capture scope, every key
    present, `{window, monitor, monitor_fallback, unknown}`; steps without a record count as `unknown`.
  - `password_checks`: object counting them per password check outcome, every key present,
    `{ok, flagged, off, unknown}` (`src-tauri/src/network/upload.rs`).
  - `pipeline_id` (optional, left out when nothing is selected): an id from `GET /pipelines`.
- `skip_pii_check` (optional): the text `true`, sent only when the check is skipped.

Response: on 2xx the body is the SSE stream below.

Status codes: 401 refreshes the token and retries once, then signs the user out. Other 4xx except
429 are not retried. 429, 5xx and network errors are retried with backoff (1, 2, 4 s).

### SSE events of `POST /generate`

Source: `src-tauri/src/network/sse.rs:88-149` (`consume_sse_stream`). Events are `event: <type>` and
`data: <json>` lines separated by a blank line. Unknown types are ignored.

| Event             | `data`                                                     | Client action |
|-------------------|------------------------------------------------------------|---------------|
| `status`          | `{"message": string}` or plain text; `{"type": "heartbeat", ...}` | Shows the message; heartbeats are skipped. |
| `job_id`          | `{"job_id": string}`                                       | Remembered, so a dropped stream can be resumed by polling `GET /generate/{job_id}/result`. |
| `result`          | `{"enriched": [object], "markdown": string, "job_id"?: string, "pdf_url"?: string}` | The final result. |
| `pii_blocked`     | `{"findings": any}`                                        | Passed to the UI; ends the stream. |
| `error`           | `{"message": string, "job_id"?: string}` or plain text     | Ends the stream with that message. |
| `resume_resubmit` | `{"job_id": string, "message": string}`                    | Passed to the UI. |

A stream that ends without `result` is an error.

### `GET /generate/{job_id}/result`

Source: `src-tauri/src/network/jobs.rs:20` (`poll_job_result`). Auth: bearer. 15 s timeout, polled
with backoff (3 s doubling, capped at 30 s).

- Response fields read: `status`, and
  - `completed`: `enriched`, `markdown`, `job_id`, `pdf_url` (same shape as the SSE `result`).
  - `processing`: keep polling.
  - `failed` or `partial`: `error` (string) is the message.
- Status codes: 401 refreshes the token and retries; other non-2xx are retried on the next attempt.

### `GET /pipelines`

Source: `src-tauri/src/network/pipelines.rs:29` (`fetch_pipelines`). Auth: bearer.

- Response: array of `{"id": string, "display_name": string, "description"?: string}`. An empty array
  means no selector. Only `id` is ever sent back (as `metadata.pipeline_id`).
- Status codes: any non-2xx is an error.

### `GET /quota`

Source: `src-tauri/src/network/quota.rs:46` (`fetch_quota`). Auth: bearer.

- Response fields read:
  - `count`, `limit`, `remaining`: integers
  - `features` (optional): `{"advanced_settings"?: bool}`
  - `generation_settings` (optional): `{"pipeline_versions"?: [int], "models"?: [string],
    "default_model"?: string}`. When absent the client assumes versions `[1, 2]` and model
    `azure/gpt-5.4`.
- Status codes: any non-2xx is an error.

### `POST /client-reports`

Source: `src-tauri/src/network/reports.rs:75` (`submit_report`). Auth: bearer. 15 s timeout.

- Request: `{schema_version, report_id, kind, occurred_at, app_version, os, os_version, arch, locale,
  phase, message, location, log_tail, settings, job_id, comment}`.
  - `schema_version`: `1`
  - `kind`: `panic`, `command_error` or `ui_error`
  - `phase`: `startup`, `login`, `idle`, `recording`, `review`, `processing`, `settings` or `unknown`
  - `log_tail`: array of strings
  - `location`, `job_id`, `comment`: string or null
  - `settings`: null or `{upload_target, pipeline_version, generation_model, hide_from_screenshots,
    skip_pii_check}`
- Response fields read: `report_id` (optional; the client falls back to its own number).
- Status codes: 401 refreshes the token and retries once; any other 2xx means handled, any other
  non-2xx is a failure and the report stays on disk.

## Web app pages

### `/desktop/authorize`

Source: `src-tauri/src/commands/auth.rs:231`; checked by the `server-contract` job in
`.github/workflows/release.yml` (it must not answer 404). The recorder opens it in the browser with
the query `redirect_uri`, `state`, `code_challenge`, `code_challenge_method=S256`. `redirect_uri` is
`http://127.0.0.1:<port>/callback`. The web app returns there with `code` and `state`, or with
`error` (`access_denied`, `server_error`).

### `/desktop/signed-in` and `/desktop/sign-in-failed`

Source: `src-tauri/src/network/loopback.rs:67-68`. The loopback listener answers the browser with a
302 to `/desktop/signed-in` on success, or to `/desktop/sign-in-failed?reason=<reason>` with
`reason` one of `verification`, `denied`, `unsupported`, `error`.

## Changing the contract

1. The change lands in this file first.
2. Then in the server, deployed to production.
3. Then in the recorder.

The `server-contract` job (`.github/workflows/release.yml:17`) holds the release tag until
production serves what the build needs; a new endpoint the app depends on gets a line there.
Whether a change may break older clients is set by the contract phase line in `AGENTS.md`.
