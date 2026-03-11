---
name: cross-repo-update-prompt
description: Create implementation prompts for a related repository based on changes already made in the current repository. Use this when the user wants a frontend/dashboard/app prompt generated from backend/API changes, or any cross-repo handoff prompt derived from commits, diffs, or local code changes.
---

# Cross-Repo Update Prompt

Use this skill when a user has already changed one repository and wants a high-quality prompt for updating a second repository.

## Outcome

Produce a prompt that another Codex agent or engineer can use immediately in the target repo.

The prompt should:
- summarize the source-repo changes accurately
- translate them into target-repo tasks
- include request/response shapes only when they affect implementation
- call out behavior changes, edge cases, and verification steps
- avoid vague requests like "support the new API"

## Workflow

1. Inspect the source repo changes first.
   - Prefer `git show --stat HEAD`, `git diff --stat`, and targeted reads of changed files.
   - Extract only the facts the target repo needs: APIs, payloads, auth, UI-visible behavior, error cases, data sync implications.

2. Infer the target repo responsibilities.
   - UI entry points
   - API client updates
   - state refresh / invalidation
   - validation and disabled states
   - error handling
   - user messaging
   - tests

3. Write the prompt for execution, not for discussion.
   - Tell the implementer to inspect the target repo first.
   - Ask for code changes, not just a plan, unless the user asked for planning.
   - Preserve the target repo's design system and existing patterns.

4. Include constraints from the source repo.
   - auth requirements
   - multipart/form-data or file handling
   - preview/commit or other multi-step flows
   - snapshot/cache consistency caveats
   - backward compatibility notes

## Prompt Template

Use this structure and fill it with concrete details:

```text
<target-repo> has to be updated to match changes in <source-repo>.

Source change summary:
- ...
- ...

Relevant API/details:
- endpoint:
- auth:
- request:
- response:
- important behavior:

Update <target-repo> to:
1. ...
2. ...
3. ...

Implementation requirements:
- inspect the existing codebase first
- follow the current API client/state/UI patterns
- preserve the existing design system
- handle loading/empty/error/success states
- include any needed type updates and tests

After implementation, summarize:
- key files changed
- user-visible flow
- important caveats
- how it was verified
```

## Quality Bar

- Prefer exact field names over paraphrases.
- Include only source-repo details that materially affect the target repo.
- If a policy mismatch matters, state it explicitly.
  - Example: `GET /budget.total_spent` may differ from `GET /budget/spending.total_spent` because snapshot values are not updated by import.
- If the source repo added a multi-step flow, make the step boundaries explicit in the prompt.
- If the source repo uses selection identifiers like fingerprints or ids, name them explicitly and explain how the target repo should use them.

## Output Style

- Default to one ready-to-send prompt in a fenced `text` block.
- If useful, add 2-4 short bullets after the prompt with customization notes.
- Do not include internal analysis unless the user asks for it.

## When To Ask Follow-Up Questions

Ask only if one of these is unknown and materially changes the prompt:
- which target repo should be updated
- whether the prompt should ask for planning only vs implementation
- whether the user wants a generic reusable prompt or one tied to a specific commit/diff

Otherwise, proceed from local repo state.
