# Experimental deployment planner

## Outcome

Create a review plan for one bounded Fasti mode. Verify the plan and understand
how to stop or remove it.

## Current support state

The planner is documentation tooling. It does not create a supported installer
or production release.

[Open the deployment planner](/deploy/)

## Available modes

- Native local development.
- Podman local review.
- Docker local review.
- Trusted HTTPS proxy, advanced and explicitly bounded.

Production deployment remains visible but unavailable until the governing B8
release and packaging evidence passes.

## Generated plan

The planner generates:

- one quoted command for an available review mode;
- the environment values used by a native mode;
- verification steps;
- bounded stop and recovery steps;
- explicit blockers for an unavailable or invalid mode.

## Secret boundary

The planner does not ask for or generate a credential. Do not put a credential,
initialization proof, or private payload in any field. The browser does not put
planner values in a URL or planner-owned browser storage. Reset restores the
documented defaults.

## Verify the result

Read the full plan before running its command. Confirm that the listener is
loopback unless an explicitly trusted HTTPS proxy is configured. Confirm that
the data root is private and durable.

## Problems and recovery

If the planner rejects a value, keep the previous state. Correct the named field
or select the local default. Do not bypass a listener, path, or proxy check.

[Read the security boundary](/security/)

Content state: STE-controlled draft.
