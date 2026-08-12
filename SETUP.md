# Camel setup — instructions for a coding agent

You are setting up **Camel**, a macOS menu-bar widget that shows how much of the
user's Claude Code subscription is left. Camel reads a file; Claude Code's
status line writes it. Your job is to connect the two, then check that it worked.

Do the steps in order. Nothing here needs network access or credentials.

## 1. Find every Claude Code config directory

Look in the user's home directory for `.claude` and any `.claude-*` sibling
(`~/.claude-work`, for instance). Each one is a separate login with its own
subscription, and Camel shows each separately.

They share a status line more often than not — `~/.claude-work/statusline.sh` is
usually a symlink to `~/.claude/statusline.sh`. Follow the symlink and edit the
real file once; do not copy it.

## 2. Make the status line save its input

Claude Code hands the status-line script a JSON on stdin that carries
`rate_limits`. Camel needs that JSON on disk, as `statusline-last.json`, inside
**the config directory of the session that produced it**.

If the user has no status line yet, create `~/.claude/statusline.sh`:

```bash
#!/usr/bin/env bash
input=$(cat)

# Where to save: the config directory this session belongs to, derived from the
# transcript path. Never a fixed path — with more than one login on the machine
# they would take turns overwriting a single file.
config=$(printf '%s' "$input" | jq -r '.transcript_path // empty' | sed -n 's|\(.*\)/projects/.*|\1|p')
[ -z "$config" ] && config="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"

# Only when limits are actually present: a freshly started session calls the
# status line before its first API reply and arrives with `rate_limits: null`,
# which would wipe the last live numbers.
if printf '%s' "$input" | jq -e '.rate_limits.five_hour.used_percentage != null' >/dev/null 2>&1; then
  printf '%s' "$input" > "$config/statusline-last.json.tmp" \
    && mv -f "$config/statusline-last.json.tmp" "$config/statusline-last.json"
fi

# Whatever the user wants to see in the terminal goes below. This prints the
# model name; replace it or keep it.
printf '%s\n' "$(printf '%s' "$input" | jq -r '.model.display_name // empty')"
```

Then `chmod +x ~/.claude/statusline.sh`.

**If a status line already exists, do not replace it.** Insert only the two
blocks above — the `config=` lines and the `if` block — near the top, right
after the script reads stdin, and leave everything the user already prints
untouched. If the script reads stdin into a differently named variable, use that
name instead of `input`.

The write is atomic (`.tmp` then `mv`) on purpose: Camel may read the file at
any moment, and a half-written JSON would show up as "found the file, but no
limits in it".

## 3. Register the status line

In `~/.claude/settings.json`, the `statusLine` key must point at the script:

```json
{
  "statusLine": {
    "type": "command",
    "command": "bash /Users/<user>/.claude/statusline.sh"
  }
}
```

Use the absolute path, with the real home directory spelled out. Keep every
other key in the file as it is. If a second config directory has its own
`settings.json` and no `statusLine`, add the same block there, pointing at the
same script.

## 4. Check that it worked

Ask the user to send one message in Claude Code — the status line runs on every
turn — and then confirm all three:

1. `statusline-last.json` exists in the config directory of that session;
2. it parses as JSON and has `.rate_limits.five_hour.used_percentage`;
3. Camel's panel shows two bars instead of the setup message (click the icon in
   the menu bar).

If the file is missing, run the script by hand with a sample payload to see the
error: `echo '{"transcript_path":"'"$HOME"'/.claude/projects/x/y.jsonl","rate_limits":{"five_hour":{"used_percentage":10,"resets_at":0},"seven_day":{"used_percentage":5,"resets_at":0}}}' | bash ~/.claude/statusline.sh`.
That must create `~/.claude/statusline-last.json`. Common causes: `jq` is not
installed (`brew install jq`), the script is not executable, or `statusLine` in
settings points somewhere else.

## What Camel does with it

Reads it, and nothing else — no network, no credentials, no telemetry. Each
config directory becomes one account in the widget: a pair of bars in the menu
bar (last 5 hours, last 7 days) and a named group in the panel. A login that has
a config but has never written the file says "No sessions yet" rather than
showing empty bars.

Do not report success until step 4 passes.
