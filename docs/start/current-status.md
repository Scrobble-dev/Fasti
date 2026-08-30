# Current status

This page separates four kinds of state. One state does not prove another.

## State dimensions

| Dimension | Question |
| --- | --- |
| Contract state | Does a governed interface define the behavior? |
| Implementation state | Does source code implement the behavior in an owned layer? |
| Runtime state | Does a real executable composition expose the behavior? |
| Support state | Does the project support the behavior for public use? |

The generated [capability reference](/reference/capabilities/) shows these
dimensions for each capability.

## What is available now

`fastid` exposes the health route. With a private data root and a permitted
listener, it also exposes durable initialization, browser-session, observation,
Record, identifier, namespace, profile-state, and Nuvio Collections routes.

Direct bootstrap is local. A non-loopback listener can expose the authenticated
non-bootstrap subset only behind an explicitly trusted HTTPS proxy.

## What is not a release

The following items are not supported public releases:

- public container images and binaries;
- a packaged desktop application;
- a supported installer;
- a supported production deployment;
- general import and replication services;
- the pre-production browser Workbench.

Identity review and B3 correction and portability paths remain staged behind
internal application ports for review.

## Safe next action

- To verify the local daemon, use [Local review](/operate/local-review/).
- To inspect an integration contract, use
  [First observation](/integrate/first-observation/).
- To evaluate a planned deployment, use the
  [experimental deployment planner](/operate/deployment-planner/).

## Source and review evidence

- [Repository status table](https://github.com/Scrobble-dev/Fasti#current-status)
- [Capability ledger](https://github.com/Scrobble-dev/Fasti/blob/dev/docs/capability-ledger.md)
- [Roadmap](https://github.com/Scrobble-dev/Fasti/blob/dev/ROADMAP.md)

Content state: STE-controlled draft.
