# Agent Notes

## Dependencies

Avoid adding dependencies. Ask the user if it truly is the best choice. Never
vendor dependencies.

## Commit Messages

Use the existing project style:

```text
Area: imperative summary
```

Examples:

```text
Fir: reject zero filter_n decimation
Delay: wait for pending delay before EOF
workflows: Add --bench
```

Commit message rules:

- Keep the subject line at or below 72 characters.
- Wrap body text at 72 characters.
- Use a body when the fixed control flow or failure mode is not obvious
  from the subject line.
- Make one logical fix per commit.
- Do not use `--no-verify`.
- Preserve unrelated user changes and untracked files.
