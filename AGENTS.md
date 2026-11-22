# ranote (fork of rnote)

## Agent skills

### Issue tracker

Issues live as GitHub issues in `mndckr/ranote`, managed with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default canonical labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` + `docs/adr/` when they exist. See `docs/agents/domain.md`.

### Live poking

Drive the running GTK app (actions, settings, colours, drawing) with `docs/agents/rnote_poke.py`. See `docs/agents/live-poking.md`.

Debug Android-only bugs on a real device over adb (build/install/logcat/synthetic input). See `docs/agents/android-debugging.md`.
