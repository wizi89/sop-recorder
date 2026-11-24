# Issue tracker

The tracker the planning skills (`to-spec`, `to-tickets`, `wayfinder`) publish to, and how the factory reads what they publish.

- **Tracker:** GitHub issues in `wizi89/sop-recorder`. The repo is public: an issue states the problem and the wanted behaviour, nothing from the private repo (no customer, pilot or pricing details). In a cloud session use REST (`gh api repos/wizi89/sop-recorder/...`), since `gh issue` fails there.
- **Glossary check:** before publishing, write the draft (title, blank line, body) to a file and run `python3 scripts/check_glossary_terms.py --text <file>`; fix every hit. The `Issue glossary` workflow runs the same check on every opened or edited issue.
- **Spec** (`to-spec`): one issue titled `Spec: <feature> (ADR-NNNN)`, or without the ADR when the grilling recorded none. It gets no `agent:*` label, because nobody builds the spec itself.
- **Tickets** (`to-tickets`): one issue per ticket, labelled `agent:explore` where the skill says `ready-for-agent`. The factory routes on `agent:explore`; `ready-for-agent` routes nothing.
- **Blocking edges:** native blocked-by (`POST .../issues/<n>/dependencies/blocked_by -F issue_id=<blocker .id>`, the REST `id`, not the number). Explore and Implement skip a ticket while it has an open blocker and pick it up once the blocker closes.
- **Owner answers:** an item with `needs-owner` or `agent:blocked` gets one comment whose first line is `/go [station] [answer]`. The Home router swaps the labels and fires the station.
- **Parent:** when the work came from an existing issue, every ticket and the spec become its sub-issues (`POST .../issues/<parent>/sub_issues -F sub_issue_id=<.id>`). The parent stays open and unchanged.

## How tickets ship

Each ticket becomes one PR into `main`, goes through Review and the owner's merge. A merge builds the installers, and the next `v*` tag ships everything merged so far to every user through the auto-updater. So each ticket has to be safe in a release on its own:

- a change the server must understand first waits until the server side is merged and deployed (`docs/server-contract.md`);
- behaviour that must stay hidden until a later ticket sits behind a setting that defaults off, which the last ticket removes;
- a ticket that touches capture, input hooks, windows, permissions or audio says in its body that the owner's hands-on check on Windows (and macOS, if macOS code changes) comes before the merge.
