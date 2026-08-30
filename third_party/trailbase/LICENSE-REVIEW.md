# TrailBase v0.33.5 licence review

- Review date: 2026-08-30
- Scope: exact, unmodified TrailBase v0.33.5 native and OCI artifacts run as a separate process
- Upstream tag: `v0.33.5`
- Upstream commit: `b4c85d5152d4e5f472e0b5da5303f7c938e3a083`
- Licence: Open Software License 3.0 (`OSL-3.0`)
- Reviewed licence SHA-256: `be4741d827008446e5e8bf9ee42f9e57b57245b6aed260fbdaf00ffebe958fb7`
- Decision: approved for this bounded integration, subject to the conditions below

The conformance package also uses exact, unmodified TrailBase `v0.33.4` native
artifacts only as an adjacent-version upgrade and rollback fixture. Its tag is
`v0.33.4` at commit `e00b2df30c3cc403f1b42fdd1af21755dd98f504`, and its included
`LICENSE` has the same reviewed SHA-256. It is not a supported runtime selection
or a modified distribution.

## Boundary

Fasti downloads or runs the exact upstream artifact. Fasti does not copy
TrailBase source into its workspace, patch the executable, link to TrailBase,
read TrailBase tables, or use TrailBase Record APIs for Fasti data. The
processes have separate data roots and communicate only through documented
public HTTP APIs.

This technical boundary is intentional. It does not, by itself, decide whether
a particular distribution is a collective work or a derivative work under
applicable law.

## Conditions

The package and any external deployment must:

1. keep TrailBase under `OSL-3.0` and retain its licence and notices;
2. identify the exact TrailBase version, source tag, source URL, and artifact
   digest;
3. provide convenient access to the corresponding upstream source for as long
   as that TrailBase artifact is distributed;
4. treat an external network deployment as distribution for the OSL conditions;
5. avoid using TrailBase names or marks to endorse Fasti;
6. keep TrailBase unmodified and separate; and
7. stop for a new written licence review before embedding, linking, patching,
   vendoring source, or distributing a modified TrailBase build.

The release archive's `LICENSE` file is checked against the reviewed digest.
The exact source remains available from the [upstream v0.33.5 tag](https://github.com/trailbaseio/trailbase/tree/v0.33.5).
The licence text is also available from the [upstream tagged licence](https://github.com/trailbaseio/trailbase/blob/v0.33.5/LICENSE)
and the [SPDX OSL-3.0 record](https://spdx.org/licenses/OSL-3.0.html).

## Release gate

This review approves the current source and development integration. Before a
public binary or OCI release, the release package must prove that the exact
licence text, attribution, corresponding-source link, and version/digest notice
are present. If counsel identifies a conflict or additional obligation, stop
distribution and update this review.

No AniList code, API, data, or trademark permission is part of this TrailBase
integration decision.
